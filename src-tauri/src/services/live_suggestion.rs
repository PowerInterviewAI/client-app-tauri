use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use parking_lot::Mutex;

use crate::consts::{API_LLM_LIVE_SUGGESTION, LIVE_SUGGESTION_NO_SUGGESTION};
use crate::services::api_client::ApiClient;
use crate::services::app_state::AppStateService;
use crate::store::ConfigStore;
use crate::types::app_state::{LiveSuggestion, Speaker, SuggestionState, Transcript};
use crate::utils::{generate_uuid, now_ms};

pub struct LiveSuggestionService {
    // BTreeMap keyed by timestamp so `.values()` always yields suggestions in
    // chronological order; the renderer relies on array order to mark the newest item.
    suggestions: Arc<Mutex<BTreeMap<i64, LiveSuggestion>>>,
    abort_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    app_state: Arc<AppStateService>,
    config_store: Arc<ConfigStore>,
}

impl LiveSuggestionService {
    pub fn new(app_state: Arc<AppStateService>, config_store: Arc<ConfigStore>) -> Self {
        Self {
            suggestions: Arc::new(Mutex::new(BTreeMap::new())),
            abort_flags: Arc::new(Mutex::new(HashMap::new())),
            app_state,
            config_store,
        }
    }

    pub fn clear(&self) {
        self.stop_running();
        self.suggestions.lock().clear();
        self.app_state.set_live_suggestions(vec![]);
    }

    pub fn stop(&self) {
        self.stop_running();
    }

    fn stop_running(&self) {
        for flag in self.abort_flags.lock().values() {
            flag.store(true, Ordering::Release);
        }
    }

    fn emit_suggestions(&self) {
        let list: Vec<LiveSuggestion> = self.suggestions.lock().values().cloned().collect();
        self.app_state.set_live_suggestions(list);
    }

    pub async fn start_generate(&self, mut transcripts: Vec<Transcript>) {
        // remove trailing SELF transcripts (same as original)
        while transcripts
            .last()
            .map(|t| matches!(t.speaker, Speaker::SelfSpeaker))
            .unwrap_or(false)
        {
            transcripts.pop();
        }
        if transcripts.is_empty() {
            return;
        }

        self.stop_running();
        self.abort_flags.lock().clear();

        let task_id = generate_uuid();
        let abort_flag = Arc::new(AtomicBool::new(false));
        self.abort_flags
            .lock()
            .insert(task_id.clone(), Arc::clone(&abort_flag));

        let conf = self.config_store.get_config();
        let token = conf.session_token.clone();
        let body = serde_json::json!({
            "config": conf.llm_conf,
            "profile_data": conf.interview_conf.profile_data,
            "context": conf.interview_conf.job_description,
            "transcripts": transcripts,
        });

        let timestamp = now_ms();
        let last_question = transcripts
            .last()
            .map(|t| t.text.clone())
            .unwrap_or_default();

        {
            let mut map = self.suggestions.lock();
            map.insert(
                timestamp,
                LiveSuggestion {
                    timestamp,
                    last_question: last_question.clone(),
                    answer: String::new(),
                    state: SuggestionState::Pending,
                    error: String::new(),
                },
            );
        }
        self.emit_suggestions();

        let suggestions = Arc::clone(&self.suggestions);
        let app_state = Arc::clone(&self.app_state);
        let abort = Arc::clone(&abort_flag);

        tokio::spawn(async move {
            let client = if token.is_empty() {
                ApiClient::new()
            } else {
                ApiClient::new().with_token(&token)
            };

            let emit = |map: &BTreeMap<i64, LiveSuggestion>| {
                let list: Vec<LiveSuggestion> = map.values().cloned().collect();
                app_state.set_live_suggestions(list);
            };

            match client.post_stream(API_LLM_LIVE_SUGGESTION, &body).await {
                Err(e) => {
                    let error_msg = crate::utils::llm_error_message(&e);
                    let mut map = suggestions.lock();
                    if let Some(s) = map.get_mut(&timestamp) {
                        s.state = SuggestionState::Error;
                        s.error = error_msg;
                    }
                    emit(&map);
                }
                Ok(resp) => {
                    let mut current = LiveSuggestion {
                        timestamp,
                        last_question: last_question.clone(),
                        answer: String::new(),
                        state: SuggestionState::Loading,
                        error: String::new(),
                    };

                    // While the (possibly partial) answer is still a prefix of the
                    // "no suggestion" sentinel, hide it from the UI; once it diverges
                    // (a real suggestion) re-show it, even if it was hidden moments ago.
                    let update = |map: &mut BTreeMap<i64, LiveSuggestion>,
                                  current: &LiveSuggestion| {
                        if !current.answer.is_empty()
                            && LIVE_SUGGESTION_NO_SUGGESTION.starts_with(&current.answer)
                        {
                            map.remove(&timestamp);
                        } else {
                            map.insert(timestamp, current.clone());
                        }
                    };

                    {
                        let mut map = suggestions.lock();
                        update(&mut map, &current);
                        emit(&map);
                    }

                    let mut stream = resp.bytes_stream();
                    let mut pending = Vec::<u8>::new();

                    while let Some(chunk) = stream.next().await {
                        if abort.load(Ordering::Acquire) {
                            current.state = SuggestionState::Stopped;
                            let mut map = suggestions.lock();
                            update(&mut map, &current);
                            emit(&map);
                            return;
                        }
                        if let Ok(bytes) = chunk {
                            pending.extend_from_slice(&bytes);
                            crate::utils::drain_utf8(&mut pending, &mut current.answer);

                            let mut map = suggestions.lock();
                            update(&mut map, &current);
                            emit(&map);
                        }
                    }

                    // finalize
                    if current.state == SuggestionState::Loading {
                        current.state = SuggestionState::Success;
                    }
                    let mut map = suggestions.lock();
                    update(&mut map, &current);
                    emit(&map);
                }
            }
        });
    }
}
