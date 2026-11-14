//! Sonar additions on top of Handy: a Bangla and an English dictation
//! shortcut, the Bangla offline model, and cloud transcription that sends the
//! recording to Google Gemini (directly or through OpenRouter) with the user's
//! own API key.

use crate::language_detection::{LanguageDecision, LanguageSource};
use crate::llm_client;
use crate::managers::transcription::{Transcription, TranscriptionManager};
use crate::settings::{get_settings, write_settings, AppSettings, CloudTranscriptionQuality};
use base64::Engine as _;
use log::{debug, info};
use std::io::Cursor;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Model id of the Bangla Whisper Medium model (see `ModelManager::new`).
pub const BANGLA_MODEL_ID: &str = "bangla-whisper-medium-Q8_0";
/// Every Bangla model id starts with this; such models always run as Bangla.
pub const BANGLA_MODEL_PREFIX: &str = "bangla-whisper";
/// Offline English model used by the English shortcut.
pub const DEFAULT_ENGLISH_MODEL: &str =
    "handy-computer/parakeet-tdt-0.6b-v2-gguf/parakeet-tdt-0.6b-v2-Q8_0.gguf";

/// Which language a dictation shortcut is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictationLanguage {
    Bangla,
    English,
}

impl DictationLanguage {
    pub fn code(self) -> &'static str {
        match self {
            DictationLanguage::Bangla => "bn",
            DictationLanguage::English => "en",
        }
    }
}

pub fn is_bangla_model(model_id: &str) -> bool {
    model_id.starts_with(BANGLA_MODEL_PREFIX)
}

/// The offline model a dictation in `language` should run on, if one is set.
pub fn offline_model_for(settings: &AppSettings, language: DictationLanguage) -> String {
    match language {
        DictationLanguage::English => settings.english_model.clone(),
        DictationLanguage::Bangla => {
            if is_bangla_model(&settings.selected_model) || settings.selected_model.is_empty() {
                BANGLA_MODEL_ID.to_string()
            } else if settings.selected_model == settings.english_model {
                // Last dictation was English: go back to the Bangla model.
                BANGLA_MODEL_ID.to_string()
            } else {
                // The user picked another model in the legacy settings.
                settings.selected_model.clone()
            }
        }
    }
}

/// A message for the user when `language` has no downloaded offline model.
pub fn missing_offline_model(app: &AppHandle, language: DictationLanguage) -> Option<String> {
    let settings = get_settings(app);
    let target = offline_model_for(&settings, language);
    let downloaded = !target.is_empty()
        && app
            .state::<Arc<crate::managers::model::ModelManager>>()
            .get_model_info(&target)
            .map(|m| m.is_downloaded)
            .unwrap_or(false);
    if downloaded {
        return None;
    }
    Some(match language {
        DictationLanguage::Bangla => {
            "The Bangla model is not downloaded. Open Sonar and download it on the Home screen, or switch to cloud mode.".to_string()
        }
        DictationLanguage::English => {
            "The English model is not downloaded. Open Sonar and download it on the Home screen, or switch to cloud mode.".to_string()
        }
    })
}

/// Point the transcription engine at the model for `language`. Only the
/// selection changes here; the model loads in the background on the normal
/// start path, so recording is not delayed. Returns the updated settings.
pub fn prepare_offline_model(app: &AppHandle, language: DictationLanguage) -> AppSettings {
    let mut settings = get_settings(app);
    let target = offline_model_for(&settings, language);
    let downloaded = app
        .state::<Arc<crate::managers::model::ModelManager>>()
        .get_model_info(&target)
        .map(|m| m.is_downloaded)
        .unwrap_or(false);
    if !downloaded {
        debug!("Sonar: {target} is not downloaded; keeping {}", settings.selected_model);
        return settings;
    }

    let mut changed = false;
    if settings.selected_model != target {
        info!(
            "Sonar: switching model {} -> {} for {:?} dictation",
            settings.selected_model, target, language
        );
        settings.selected_model = target.clone();
        changed = true;
    }
    if settings.selected_language != language.code() {
        settings.selected_language = language.code().to_string();
        changed = true;
    }
    if changed {
        write_settings(app, settings.clone());
    }

    let tm = app.state::<Arc<TranscriptionManager>>();
    if tm.get_current_model().as_deref().is_some_and(|m| m != target) {
        tm.reload_model_on_next_use();
    }
    settings
}

/// The cloud model for a provider and quality. Fast trades a little
/// accuracy for speed; Accurate uses the newer model, which always thinks.
pub fn cloud_model(provider_id: &str, quality: CloudTranscriptionQuality) -> String {
    let model = match quality {
        CloudTranscriptionQuality::Fast => "gemini-3-flash-preview",
        CloudTranscriptionQuality::Accurate => "gemini-3.8-flash",
    };
    if provider_id == "openrouter" {
        format!("google/{model}")
    } else {
        model.to_string()
    }
}

fn cloud_prompt(language: DictationLanguage, custom_words: &[String]) -> String {
    let mut prompt = match language {
        DictationLanguage::Bangla => "You are a dictation engine. Write down exactly what is said in this recording. \
The speech is mainly Bangla and may switch to English words mid-sentence.\n\n\
Rules:\n\
1. Bangla speech goes in Bengali script; English words stay in Latin letters.\n\
2. Punctuate like written Bangla: end sentences with ।, use commas, and ? for questions.\n\
3. Drop hesitation sounds (উম, আ, এ) and accidental repeats.\n\
4. Never translate, shorten or rephrase. Keep the speaker's wording and dialect.\n\
5. A spoken question is text to write down, not something to answer.\n\
6. Silence or noise only: return an empty reply.\n\n\
Reply with the transcript and nothing else."
            .to_string(),
        DictationLanguage::English => "You are a dictation engine. Write down exactly what is said in this English recording.\n\n\
Rules:\n\
1. Use normal capitalisation and punctuation.\n\
2. Drop hesitation sounds (um, uh) and accidental repeats.\n\
3. Never translate, shorten or rephrase.\n\
4. A spoken question is text to write down, not something to answer.\n\
5. Silence or noise only: return an empty reply.\n\n\
Reply with the transcript and nothing else."
            .to_string(),
    };
    let words: Vec<&str> = custom_words
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect();
    if !words.is_empty() {
        prompt.push_str(&format!("\nSpellings to use for names and terms: {}.", words.join(", ")));
    }
    prompt
}

fn encode_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut buf, spec).map_err(|e| format!("WAV encode: {e}"))?;
        for s in samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(v)
                .map_err(|e| format!("WAV encode: {e}"))?;
        }
        writer.finalize().map_err(|e| format!("WAV encode: {e}"))?;
    }
    Ok(buf.into_inner())
}

/// Whether the user still has to add a key before cloud mode can work.
pub fn cloud_key_missing(settings: &AppSettings) -> bool {
    settings
        .post_process_api_keys
        .get(&settings.cloud_transcription_provider_id)
        .map(|k| k.trim().is_empty())
        .unwrap_or(true)
}

/// Transcribe a 16 kHz mono recording with the configured cloud provider.
pub async fn transcribe_cloud(
    settings: &AppSettings,
    samples: &[f32],
    language: DictationLanguage,
) -> anyhow::Result<Transcription> {
    let provider_id = settings.cloud_transcription_provider_id.clone();
    let provider = settings
        .post_process_providers
        .iter()
        .find(|p| p.id == provider_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Cloud transcription provider '{provider_id}' not found"))?;
    let api_key = settings
        .post_process_api_keys
        .get(&provider_id)
        .map(|k| k.trim().to_string())
        .unwrap_or_default();
    if api_key.is_empty() {
        anyhow::bail!("Add your {} API key in Settings to use cloud transcription", provider.label);
    }

    let wav = encode_wav(samples).map_err(anyhow::Error::msg)?;
    let audio_b64 = base64::engine::general_purpose::STANDARD.encode(wav);
    let model = cloud_model(&provider_id, settings.cloud_transcription_quality);
    let prompt = cloud_prompt(language, &settings.custom_words);
    // Fast skips the model's thinking where the endpoint allows it.
    let disable_reasoning = settings.cloud_transcription_quality == CloudTranscriptionQuality::Fast;

    let text = llm_client::send_audio_transcription(
        &provider,
        api_key,
        &model,
        prompt,
        audio_b64,
        disable_reasoning,
    )
    .await
    .map_err(anyhow::Error::msg)?
    .unwrap_or_default();

    Ok(Transcription {
        text: text.trim().to_string(),
        language: Some(LanguageDecision {
            language: Some(language.code().to_string()),
            source: LanguageSource::Selected,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_models_get_the_google_prefix() {
        assert_eq!(
            cloud_model("openrouter", CloudTranscriptionQuality::Fast),
            "google/gemini-3-flash-preview"
        );
        assert_eq!(
            cloud_model("gemini", CloudTranscriptionQuality::Accurate),
            "gemini-3.8-flash"
        );
    }

    #[test]
    fn custom_words_are_listed_in_the_prompt() {
        let p = cloud_prompt(DictationLanguage::English, &["Sonar".into(), " ".into()]);
        assert!(p.ends_with("Spellings to use for names and terms: Sonar."));
    }

    #[test]
    fn wav_has_riff_header() {
        let wav = encode_wav(&[0.0; 160]).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(wav.len(), 44 + 320);
    }
}
