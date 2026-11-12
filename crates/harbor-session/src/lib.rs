pub mod resume;
pub mod sources;
pub mod vault;

pub use resume::{past_ninety, should_offer, Resume, ResumeStore, OFFER_AFTER_NS};
pub use sources::{parse_smb_url, smb_id, validate_smb, SourceConfig, SourceStore};
pub use vault::CredentialVault;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub subtitle_lang: String,
    pub audio_lang: String,
    pub seek_step_s: u32,
    pub ui_lang: harbor_platform::Lang,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            subtitle_lang: "en".into(),
            audio_lang: "und".into(),
            seek_step_s: 10,
            ui_lang: harbor_platform::Lang::En,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seek_step() {
        assert_eq!(Settings::default().seek_step_s, 10);
        assert_eq!(Settings::default().ui_lang, harbor_platform::Lang::En);
    }
}
