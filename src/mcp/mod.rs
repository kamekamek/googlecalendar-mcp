use std::sync::Arc;

use crate::google_calendar::{
    CalendarEvent, EventPayload, GetEventParams, ListEventsParams, ListEventsResponse,
};
use crate::oauth::TokenInfo;
use crate::token_ingest::{ingest_bearer_token_from_headers, BearerTokenError};
use crate::AppState;
use axum::http::request::Parts;
use rmcp::{
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Extensions, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct CalendarService {
    state: Arc<AppState>,
    tool_router: ToolRouter<Self>,
}

impl CalendarService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    async fn ensure_token(
        &self,
        user_id: &str,
        extensions: &Extensions,
    ) -> Result<TokenInfo, ErrorData> {
        if let Some(parts) = extensions.get::<Parts>() {
            if let Err(err) =
                ingest_bearer_token_from_headers(&self.state, &parts.headers, user_id).await
            {
                match err {
                    BearerTokenError::InvalidUtf8(source) => {
                        return Err(ErrorData::invalid_request(
                            format!("authorization header must be valid UTF-8: {source}"),
                            None,
                        ));
                    }
                    BearerTokenError::Storage(source) => {
                        return Err(internal_error("token persist", source));
                    }
                }
            }
        }

        let mut token = match self
            .state
            .token_storage
            .fetch(user_id)
            .await
            .map_err(|err| internal_error("token fetch", err))?
        {
            Some(token) => token,
            None => {
                return Err(ErrorData::invalid_request(
                    format!("user '{user_id}' is not authorized; complete OAuth flow"),
                    None,
                ));
            }
        };

        if token.is_expired() {
            let refresh_token = token.refresh_token.clone().ok_or_else(|| {
                ErrorData::invalid_request(
                    format!(
                        "access token for user '{user_id}' is expired and lacks a refresh token"
                    ),
                    None,
                )
            })?;

            token = self
                .state
                .oauth_client
                .refresh_access_token(&refresh_token)
                .await
                .map_err(|err| internal_error("token refresh", err))?;

            self.state
                .token_storage
                .persist(user_id, &token)
                .await
                .map_err(|err| internal_error("token persist", err))?;
        }

        Ok(token)
    }

    /// 各操作に必要な最小スコープを返す
    fn required_scope_for_operation(operation: &str) -> &'static str {
        match operation {
            "list_events" | "get_event" => {
                // Read操作 - calendar.eventsスコープが必要
                "https://www.googleapis.com/auth/calendar.events"
            }
            "create_event" | "update_event" => {
                // Write操作 - calendar.eventsスコープが必要
                "https://www.googleapis.com/auth/calendar.events"
            }
            _ => "https://www.googleapis.com/auth/calendar.events",
        }
    }

    /// Google Calendar APIエラーをMCPエラーに変換（insufficient_scope検出付き）
    fn handle_calendar_error(operation: &str, err: anyhow::Error) -> ErrorData {
        let err_str = err.to_string();
        if err_str.starts_with("insufficient_scope:") {
            let description = err_str
                .strip_prefix("insufficient_scope:")
                .unwrap_or("")
                .trim();
            let required_scope = Self::required_scope_for_operation(operation);

            ErrorData::invalid_request(
                format!("Insufficient OAuth scope: {}", description),
                Some(serde_json::json!({
                    "__mcp_oauth_error": "insufficient_scope",
                    "required_scope": required_scope,
                    "description": description,
                    "operation": operation
                })),
            )
        } else {
            internal_error(operation, err)
        }
    }
}

#[tool_router]
impl CalendarService {
    #[tool(
        name = "google_calendar_list_events",
        description = "List calendar events for the authorized user",
        annotations(
            title = "List Calendar Events",
            read_only_hint = true,
            destructive_hint = false
        )
    )]
    pub async fn list_events(
        &self,
        extensions: Extensions,
        Parameters(ListEventsInput { user_id, params }): Parameters<ListEventsInput>,
    ) -> Result<Json<ListEventsResponse>, ErrorData> {
        let token = self.ensure_token(&user_id, &extensions).await?;
        let data = self
            .state
            .google_calendar
            .list_events(&token, &params)
            .await
            .map_err(|err| Self::handle_calendar_error("list_events", err))?;
        Ok(Json(data))
    }

    #[tool(
        name = "google_calendar_get_event",
        description = "Fetch a single calendar event by ID",
        annotations(
            title = "Get Calendar Event",
            read_only_hint = true,
            destructive_hint = false
        )
    )]
    pub async fn get_event(
        &self,
        extensions: Extensions,
        Parameters(GetEventInput {
            user_id,
            event_id,
            calendar_id,
        }): Parameters<GetEventInput>,
    ) -> Result<Json<CalendarEvent>, ErrorData> {
        let token = self.ensure_token(&user_id, &extensions).await?;
        let params = GetEventParams {
            event_id,
            calendar_id,
        };
        let event = self
            .state
            .google_calendar
            .get_event(&token, &params)
            .await
            .map_err(|err| Self::handle_calendar_error("get_event", err))?;
        Ok(Json(event))
    }

    #[tool(
        name = "google_calendar_create_event",
        description = "Create a new calendar event",
        annotations(
            title = "Create Calendar Event",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    pub async fn create_event(
        &self,
        extensions: Extensions,
        Parameters(CreateEventInput { user_id, payload }): Parameters<CreateEventInput>,
    ) -> Result<Json<CalendarEvent>, ErrorData> {
        let token = self.ensure_token(&user_id, &extensions).await?;
        let event = self
            .state
            .google_calendar
            .create_event(&token, &payload)
            .await
            .map_err(|err| Self::handle_calendar_error("create_event", err))?;
        Ok(Json(event))
    }

    #[tool(
        name = "google_calendar_update_event",
        description = "Update an existing calendar event (no deletion)",
        annotations(
            title = "Update Calendar Event",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    pub async fn update_event(
        &self,
        extensions: Extensions,
        Parameters(UpdateEventInput {
            user_id,
            event_id,
            payload,
        }): Parameters<UpdateEventInput>,
    ) -> Result<Json<CalendarEvent>, ErrorData> {
        let token = self.ensure_token(&user_id, &extensions).await?;
        let event = self
            .state
            .google_calendar
            .update_event(&token, &event_id, &payload)
            .await
            .map_err(|err| Self::handle_calendar_error("update_event", err))?;
        Ok(Json(event))
    }
}

#[tool_handler]
impl ServerHandler for CalendarService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "mcp-google-calendar".into(),
                title: Some("Google Calendar MCP Bridge".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: Some("https://modelcontextprotocol.io/".into()),
            },
            instructions: Some(
                "Complete OAuth authorization before calling tools. Event deletion is intentionally disabled.".into(),
            ),
        }
    }
}

fn internal_error(operation: &str, err: anyhow::Error) -> ErrorData {
    ErrorData::internal_error(
        format!("{operation} failed: {err}"),
        Some(serde_json::json!({ "cause": err.to_string() })),
    )
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct ListEventsInput {
    /// Stable user identifier on behalf of which the request is executed.
    pub user_id: String,
    #[serde(flatten)]
    pub params: ListEventsParams,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct GetEventInput {
    pub user_id: String,
    pub event_id: String,
    #[serde(default)]
    pub calendar_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CreateEventInput {
    pub user_id: String,
    #[serde(flatten)]
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct UpdateEventInput {
    pub user_id: String,
    pub event_id: String,
    #[serde(flatten)]
    pub payload: EventPayload,
}

pub fn service_factory(state: Arc<AppState>) -> impl Fn() -> CalendarService + Clone {
    move || CalendarService::new(state.clone())
}

pub struct HttpMcpServer {
    service: CalendarService,
}

impl HttpMcpServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            service: CalendarService::new(state),
        }
    }

    pub async fn handle_request(&self, request: ToolRequest) -> ToolResponse {
        match self.try_handle(request).await {
            Ok(value) => value,
            Err(err) => {
                let message = err.message.to_string();
                let data = err.data;
                let mut response = ToolResponse::error(message);
                if let Some(data) = data {
                    response.data = Some(data);
                }
                response
            }
        }
    }

    async fn try_handle(&self, request: ToolRequest) -> Result<ToolResponse, ErrorData> {
        match request {
            ToolRequest::List { user_id, params } => {
                let Json(payload) = self
                    .service
                    .list_events(
                        Extensions::default(),
                        Parameters(ListEventsInput { user_id, params }),
                    )
                    .await?;
                let value = serde_json::to_value(payload)
                    .map_err(|err| internal_error("serialize list events", err.into()))?;
                Ok(ToolResponse::success(value))
            }
            ToolRequest::Get {
                user_id,
                event_id,
                calendar_id,
            } => {
                let Json(payload) = self
                    .service
                    .get_event(
                        Extensions::default(),
                        Parameters(GetEventInput {
                            user_id,
                            event_id,
                            calendar_id,
                        }),
                    )
                    .await?;
                let value = serde_json::to_value(payload)
                    .map_err(|err| internal_error("serialize get event", err.into()))?;
                Ok(ToolResponse::success(value))
            }
            ToolRequest::Create { user_id, payload } => {
                let Json(payload) = self
                    .service
                    .create_event(
                        Extensions::default(),
                        Parameters(CreateEventInput { user_id, payload }),
                    )
                    .await?;
                let value = serde_json::to_value(payload)
                    .map_err(|err| internal_error("serialize create event", err.into()))?;
                Ok(ToolResponse::success(value))
            }
            ToolRequest::Update {
                user_id,
                event_id,
                payload,
            } => {
                let Json(payload) = self
                    .service
                    .update_event(
                        Extensions::default(),
                        Parameters(UpdateEventInput {
                            user_id,
                            event_id,
                            payload,
                        }),
                    )
                    .await?;
                let value = serde_json::to_value(payload)
                    .map_err(|err| internal_error("serialize update event", err.into()))?;
                Ok(ToolResponse::success(value))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ToolRequest {
    List {
        user_id: String,
        #[serde(default)]
        params: ListEventsParams,
    },
    Get {
        user_id: String,
        event_id: String,
        #[serde(default)]
        calendar_id: Option<String>,
    },
    Create {
        user_id: String,
        payload: EventPayload,
    },
    Update {
        user_id: String,
        event_id: String,
        payload: EventPayload,
    },
}

#[derive(Debug, Serialize)]
pub struct ToolResponse {
    pub status: ResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResponse {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            status: ResponseStatus::Success,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            status: ResponseStatus::Error,
            data: None,
            error: Some(message),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResponseStatus {
    Success,
    Error,
}
