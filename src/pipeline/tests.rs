use super::*;
use crate::error::{LlmError, LlmResult, VoiceError};
use crate::llm::GenerationResponse;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

async fn run_llm(attempts: Vec<LlmAttempt>) -> String {
    generate_with_fallback(&mut attempts.into_iter()).await
}

async fn run_tts(providers: Vec<TtsCandidate>, text: &str) -> Result<()> {
    speak_with_fallback(&mut providers.into_iter(), text).await
}

type Calls = Arc<Mutex<Vec<String>>>;

struct FakeLlm {
    name: &'static str,
    available: bool,
    reply: Option<&'static str>,
    calls: Calls,
}

#[async_trait]
impl LlmProvider for FakeLlm {
    fn name(&self) -> &str {
        self.name
    }
    fn is_available(&self) -> bool {
        self.available
    }
    async fn generate(&self, _request: &GenerationRequest) -> LlmResult<GenerationResponse> {
        self.calls.lock().unwrap().push(self.name.to_string());
        match self.reply {
            Some(text) => Ok(GenerationResponse {
                text: text.to_string(),
                input_tokens: 0,
                output_tokens: 0,
            }),
            None => Err(LlmError::Request("boom".into())),
        }
    }
}

fn llm(
    name: &'static str,
    available: bool,
    reply: Option<&'static str>,
    calls: &Calls,
) -> (
    std::result::Result<Box<dyn LlmProvider>, String>,
    GenerationRequest,
) {
    let provider: Box<dyn LlmProvider> = Box::new(FakeLlm {
        name,
        available,
        reply,
        calls: calls.clone(),
    });
    (Ok(provider), request())
}

fn request() -> GenerationRequest {
    GenerationRequest {
        system_message: None,
        prompt: "p".into(),
        max_tokens: 10,
        temperature: 0.0,
        disable_thinking: false,
    }
}

#[tokio::test]
async fn llm_failing_provider_falls_through_to_next() {
    let calls = Calls::default();
    let out = run_llm(vec![
        llm("a", true, None, &calls),
        llm("b", true, Some("  summary \n"), &calls),
    ])
    .await;
    assert_eq!(out, "summary");
    assert_eq!(*calls.lock().unwrap(), ["a", "b"]);
}

#[tokio::test]
async fn llm_unavailable_and_uncreatable_providers_are_skipped() {
    let calls = Calls::default();
    let out = run_llm(vec![
        (Err("a: no key".to_string()), request()),
        llm("b", false, Some("never"), &calls),
        llm("c", true, Some("ok"), &calls),
    ])
    .await;
    assert_eq!(out, "ok");
    assert_eq!(*calls.lock().unwrap(), ["c"]);
}

#[tokio::test]
async fn llm_all_failing_returns_empty_string() {
    let calls = Calls::default();
    let out = run_llm(vec![
        llm("a", true, None, &calls),
        llm("b", false, Some("x"), &calls),
    ])
    .await;
    assert_eq!(out, "");
    assert_eq!(*calls.lock().unwrap(), ["a"]);
}

#[tokio::test]
async fn llm_first_success_stops_the_chain() {
    let calls = Calls::default();
    run_llm(vec![
        llm("a", true, Some("one"), &calls),
        llm("b", true, Some("two"), &calls),
    ])
    .await;
    assert_eq!(*calls.lock().unwrap(), ["a"]);
}

struct FakeTts {
    name: &'static str,
    available: bool,
    ok: bool,
    tags: bool,
    spoken: Calls,
}

#[async_trait]
impl TtsProvider for FakeTts {
    fn name(&self) -> &str {
        self.name
    }
    fn is_available(&self) -> bool {
        self.available
    }
    async fn speak(&self, text: &str) -> Result<bool> {
        self.spoken
            .lock()
            .unwrap()
            .push(format!("{}:{}", self.name, text));
        if self.ok {
            Ok(true)
        } else {
            Err(VoiceError::Config("tts boom".into()))
        }
    }
    fn supports_audio_tags(&self) -> bool {
        self.tags
    }
}

fn tts(
    name: &'static str,
    available: bool,
    ok: bool,
    tags: bool,
    spoken: &Calls,
) -> std::result::Result<Box<dyn TtsProvider>, String> {
    Ok(Box::new(FakeTts {
        name,
        available,
        ok,
        tags,
        spoken: spoken.clone(),
    }))
}

#[tokio::test]
async fn tts_failing_provider_falls_through_to_next() {
    let spoken = Calls::default();
    run_tts(
        vec![
            tts("a", true, false, false, &spoken),
            tts("b", true, true, false, &spoken),
            tts("c", true, true, false, &spoken),
        ],
        "hi",
    )
    .await
    .unwrap();
    assert_eq!(*spoken.lock().unwrap(), ["a:hi", "b:hi"]);
}

#[tokio::test]
async fn tts_unavailable_and_uncreatable_providers_are_skipped() {
    let spoken = Calls::default();
    run_tts(
        vec![
            Err("a: bad config".to_string()),
            tts("b", false, true, false, &spoken),
            tts("c", true, true, false, &spoken),
        ],
        "hi",
    )
    .await
    .unwrap();
    assert_eq!(*spoken.lock().unwrap(), ["c:hi"]);
}

#[tokio::test]
async fn tts_nothing_available_is_an_error() {
    let spoken = Calls::default();
    let err = run_tts(
        vec![
            Err("a: bad config".to_string()),
            tts("b", false, true, false, &spoken),
        ],
        "hi",
    )
    .await
    .expect_err("a chain with nothing to try must not succeed silently")
    .to_string();
    assert!(
        err.contains("No TTS provider available"),
        "unexpected: {err}"
    );
    assert!(err.contains("a: bad config") && err.contains("b: not available"));
    assert!(spoken.lock().unwrap().is_empty());
}

#[tokio::test]
async fn tts_total_failure_is_silent_ok() {
    let spoken = Calls::default();
    let result = run_tts(
        vec![
            tts("a", true, false, false, &spoken),
            Err("b: bad".to_string()),
        ],
        "hi",
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(*spoken.lock().unwrap(), ["a:hi"]);
}

#[tokio::test]
async fn tts_audio_tag_stripped_only_for_providers_without_tag_support() {
    let spoken = Calls::default();
    run_tts(
        vec![
            tts("eleven", true, false, true, &spoken),
            tts("plain", true, true, false, &spoken),
        ],
        "[satisfied] done",
    )
    .await
    .unwrap();
    assert_eq!(
        *spoken.lock().unwrap(),
        ["eleven:[satisfied] done", "plain:done"]
    );
}

fn provider_named(name: &str) -> TtsProviderConfig {
    toml::from_str(&format!("name = \"{name}\"")).unwrap()
}

#[test]
fn test_select_engine() {
    let providers = vec![provider_named("my_voice")];
    assert!(matches!(
        select_engine("", &providers),
        Ok(EngineChoice::Engine(TtsEngine::Auto))
    ));
    assert!(matches!(
        select_engine("auto", &providers),
        Ok(EngineChoice::Engine(TtsEngine::Auto))
    ));
    assert!(matches!(
        select_engine("say", &providers),
        Ok(EngineChoice::Engine(TtsEngine::MacOS))
    ));
    assert!(matches!(
        select_engine("my_voice", &providers),
        Ok(EngineChoice::Named(p)) if p.name == "my_voice"
    ));
    let err = select_engine("nonsense", &providers).err().unwrap();
    assert!(err.to_string().contains("nonsense"));
}
