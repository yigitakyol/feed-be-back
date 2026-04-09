use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use df::tract::{DfParams, DfTract, RuntimeParams};
use ndarray::prelude::*;
use ringbuf::{HeapRb, Producer, Consumer};

// ==============================================================================
// 🛠️ VST KİMLİĞİ
// ==============================================================================
const PLUGIN_NAME: &'static str = "Feed Be-Back";
const VST3_UID_16_CHAR: [u8; 16] = *b"FeedBeBackFinal1";
const DEFAULT_REDUCTION: f32 = 10.0;

// ==============================================================================
// 🧠 AI AYARLARI
// ==============================================================================
const POST_FILTER_BETA: f32 = 0.02;

const INTERFACE_CHANNELS: usize = 2;

struct DeepFilterVst {
    params: Arc<DeepFilterParams>,
    /// UI slider → Process thread doğrudan köprüsü (DAW bypass)
    shared_reduction: Arc<AtomicU32>,
    df: Option<DfTract>,
    in_prod: Vec<Producer<f32, Arc<HeapRb<f32>>>>,
    in_cons: Vec<Consumer<f32, Arc<HeapRb<f32>>>>,
    out_prod: Vec<Producer<f32, Arc<HeapRb<f32>>>>,
    out_cons: Vec<Consumer<f32, Arc<HeapRb<f32>>>>,
    inframe: Array2<f32>,
    outframe: Array2<f32>,
}

#[derive(Params)]
struct DeepFilterParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "atten"]
    pub atten_lim: FloatParam,
}

impl Default for DeepFilterVst {
    fn default() -> Self {
        Self {
            params: Arc::new(DeepFilterParams::default()),
            shared_reduction: Arc::new(AtomicU32::new(DEFAULT_REDUCTION.to_bits())),
            df: None,
            in_prod: Vec::new(),
            in_cons: Vec::new(),
            out_prod: Vec::new(),
            out_cons: Vec::new(),
            inframe: Array2::zeros((INTERFACE_CHANNELS, 480)),
            outframe: Array2::zeros((INTERFACE_CHANNELS, 480)),
        }
    }
}

impl Default for DeepFilterParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(420, 330),
            atten_lim: FloatParam::new(
                "Reduction",
                10.0,
                FloatRange::Linear { min: 0.0, max: 20.0 },
            )
            .with_step_size(1.0)
            .with_value_to_string(std::sync::Arc::new(|v| format!("{}", v as i32))),
        }
    }
}

impl Plugin for DeepFilterVst {
    const NAME: &'static str = PLUGIN_NAME;
    const VENDOR: &'static str = "Yiğit Akyol";
    const URL: &'static str = "https://yigitakyol.com";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = "1.0.0";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: std::num::NonZeroU32::new(2),
            main_output_channels: std::num::NonZeroU32::new(2),
            aux_input_ports: &[],
            aux_output_ports: &[],
            names: PortNames::const_default(),
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let shared = self.shared_reduction.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    // Başlıklar
                    ui.add_space(15.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("yigitakyol.com")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(100, 200, 255)),
                        );
                        ui.add_space(3.0);
                        ui.heading(
                            egui::RichText::new("FEED BE-BACK")
                                .size(34.0)
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new("Feedback Suppression Engine")
                                .size(13.0)
                                .color(egui::Color32::GRAY),
                        );
                    });

                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Reduction").size(16.0));
                    });
                    ui.add_space(10.0);

                    // Slider değerini egui'nin kendi hafızasında sakla.
                    let slider_id = ui.id().with("reduction_slider");
                    let mut val: f32 = ui.data_mut(|d| {
                        d.get_temp::<f32>(slider_id)
                            .unwrap_or(DEFAULT_REDUCTION)
                    });

                    let slider = egui::Slider::new(&mut val, 0.0..=20.0)
                        .step_by(1.0)
                        .text("")
                        .custom_formatter(|v, _| format!("{}", v as i32));

                    // Slider'ı ortalamak için vertical_centered + geniş slider_width
                    ui.vertical_centered(|ui| {
                        ui.spacing_mut().slider_width = 360.0;
                        ui.add(slider);
                    });

                    // Her frame'de egui hafızasını tazele
                    ui.data_mut(|d| d.insert_temp(slider_id, val));

                    // Değer değiştiyse DOĞRUDAN atomik değere yaz
                    // (process() bunu okuyacak — DAW araya giremez)
                    if val != f32::from_bits(shared.load(Ordering::Relaxed)) {
                        shared.store(val.to_bits(), Ordering::Relaxed);
                    }

                    ui.add_space(15.0);
                    ui.vertical_centered(|ui| {
                        if val < 0.5 {
                            ui.label(
                                egui::RichText::new("⏸ BYPASS")
                                    .size(20.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(255, 180, 50)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("● ACTIVE")
                                    .size(18.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(80, 220, 160)),
                            );
                        }
                    });
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        let df_params = DfParams::default();
        let r_params = RuntimeParams::default_with_ch(INTERFACE_CHANNELS);

        let mut df = match DfTract::new(df_params, &r_params) {
            Ok(engine) => engine,
            Err(_) => return false,
        };
        df.set_pf_beta(POST_FILTER_BETA);

        let hop_size = df.hop_size;
        let rb_capacity = hop_size * 4 + buffer_config.max_buffer_size as usize;

        self.in_prod.clear();
        self.in_cons.clear();
        self.out_prod.clear();
        self.out_cons.clear();

        for _ in 0..INTERFACE_CHANNELS {
            let rb_in = HeapRb::<f32>::new(rb_capacity);
            let (prod_in, cons_in) = rb_in.split();
            self.in_prod.push(prod_in);
            self.in_cons.push(cons_in);

            let rb_out = HeapRb::<f32>::new(rb_capacity);
            let (mut prod_out, cons_out) = rb_out.split();
            for _ in 0..hop_size {
                let _ = prod_out.push(0.0);
            }
            self.out_prod.push(prod_out);
            self.out_cons.push(cons_out);
        }

        self.inframe = Array2::zeros((INTERFACE_CHANNELS, hop_size));
        self.outframe = Array2::zeros((INTERFACE_CHANNELS, hop_size));
        context.set_latency_samples(hop_size as u32);
        self.df = Some(df);
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let df = match self.df.as_mut() {
            Some(engine) => engine,
            None => return ProcessStatus::Normal,
        };

        // Girişi ring buffer'a yaz
        for (ch_idx, channel_data) in buffer.as_slice().iter().enumerate() {
            if ch_idx < INTERFACE_CHANNELS {
                self.in_prod[ch_idx].push_slice(channel_data);
            }
        }

        let hop_size = df.hop_size;

        // DeepFilterNet ile işle
        while self.in_cons[0].len() >= hop_size {
            for i_ch in 0..INTERFACE_CHANNELS {
                let mut channel_view = self.inframe.row_mut(i_ch);
                for i in 0..hop_size {
                    channel_view[i] = self.in_cons[i_ch].pop().unwrap_or(0.0);
                }
            }

            // UI slider'dan doğrudan atomik okuma (DAW müdahale edemez)
            let atten = f32::from_bits(self.shared_reduction.load(Ordering::Relaxed));
            df.set_atten_lim(atten);

            let _ = df.process(self.inframe.view(), self.outframe.view_mut());

            for i_ch in 0..INTERFACE_CHANNELS {
                let channel_view = self.outframe.row(i_ch);
                for i in 0..hop_size {
                    let _ = self.out_prod[i_ch].push(channel_view[i]);
                }
            }
        }

        // Çıkışı buffer'a yaz
        let required_samples = buffer.samples();
        for (ch_idx, channel_data) in buffer.as_slice().iter_mut().enumerate() {
            if ch_idx < INTERFACE_CHANNELS {
                let available = self.out_cons[ch_idx].len();
                let to_read = std::cmp::min(required_samples, available);
                if to_read < required_samples {
                    for sample in channel_data.iter_mut() {
                        *sample = self.out_cons[ch_idx].pop().unwrap_or(0.0);
                    }
                } else {
                    let _ = self.out_cons[ch_idx].pop_slice(channel_data);
                }
            }
        }

        ProcessStatus::Normal
    }
}

unsafe impl Send for DeepFilterVst {}
unsafe impl Sync for DeepFilterVst {}

impl ClapPlugin for DeepFilterVst {
    const CLAP_ID: &'static str = "com.yigitakyol.feedbeback";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Feed Be-Back Feedback Suppression");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Utility];
}

impl Vst3Plugin for DeepFilterVst {
    const VST3_CLASS_ID: [u8; 16] = VST3_UID_16_CHAR;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Tools,
    ];
}

nih_export_clap!(DeepFilterVst);
nih_export_vst3!(DeepFilterVst);
