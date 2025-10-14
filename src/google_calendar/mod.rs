use crate::oauth::TokenInfo;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Clone)]
pub struct GoogleCalendarClient {
    http: Client,
    api_base: Url,
    default_calendar_id: Option<String>,
}

impl GoogleCalendarClient {
    pub fn new(api_base: String) -> Self {
        let http = Client::builder()
            .user_agent("mcp-google-calendar/0.1.0")
            .build()
            .expect("reqwest client");

        let mut normalized = api_base.trim().to_owned();
        if !normalized.ends_with('/') {
            normalized.push('/');
        }

        let api_base = Url::parse(&normalized).expect("valid api base url");

        Self {
            http,
            api_base,
            default_calendar_id: None,
        }
    }

    pub fn with_default_calendar(mut self, calendar_id: Option<String>) -> Self {
        self.default_calendar_id = calendar_id;
        self
    }

    fn calendar_url(&self, calendar_id: &str, path: &str) -> Result<Url> {
        let encoded_calendar = urlencoding::encode(calendar_id);
        let joined = self
            .api_base
            .join(&format!("calendars/{encoded_calendar}/{path}"))
            .context("failed to compose calendar endpoint")?;
        Ok(joined)
    }

    fn resolve_calendar<'a>(&'a self, override_id: &'a Option<String>) -> &'a str {
        override_id
            .as_deref()
            .or(self.default_calendar_id.as_deref())
            .unwrap_or("primary")
    }

    pub async fn list_events(
        &self,
        token: &TokenInfo,
        params: &ListEventsParams,
    ) -> Result<ListEventsResponse> {
        let calendar_id = self.resolve_calendar(&params.calendar_id);
        let url = self.calendar_url(calendar_id, "events")?;
        let mut request = self.http.get(url).bearer_auth(&token.access_token);

        let mut query: HashMap<&str, String> = HashMap::new();
        if let Some(time_min) = params.time_min {
            query.insert("timeMin", time_min.to_rfc3339());
        }
        if let Some(time_max) = params.time_max {
            query.insert("timeMax", time_max.to_rfc3339());
        }
        if let Some(max_results) = params.max_results {
            query.insert("maxResults", max_results.to_string());
        }
        if let Some(page_token) = &params.page_token {
            query.insert("pageToken", page_token.clone());
        }
        if let Some(query_string) = &params.query {
            query.insert("q", query_string.clone());
        }
        if params.single_events {
            query.insert("singleEvents", "true".into());
        }
        if params.order_by_start_time {
            query.insert("orderBy", "startTime".into());
        }

        if !query.is_empty() {
            request = request.query(&query);
        }

        let response = request.send().await?.error_for_status()?;
        let payload = response.json::<ListEventsResponse>().await?;
        Ok(payload)
    }

    pub async fn get_event(
        &self,
        token: &TokenInfo,
        params: &GetEventParams,
    ) -> Result<CalendarEvent> {
        let calendar_id = self.resolve_calendar(&params.calendar_id);
        let url = self.calendar_url(calendar_id, &format!("events/{}", params.event_id))?;
        let response = self
            .http
            .get(url)
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<CalendarEvent>().await?)
    }

    pub async fn create_event(
        &self,
        token: &TokenInfo,
        request: &EventPayload,
    ) -> Result<CalendarEvent> {
        let calendar_id = self.resolve_calendar(&request.calendar_id);
        let url = self.calendar_url(calendar_id, "events")?;

        let mut body = serde_json::to_value(request)?
            .as_object()
            .cloned()
            .unwrap_or_default();
        body.remove("calendar_id");
        if !body.contains_key("summary") {
            return Err(anyhow!("summary is required to create an event"));
        }
        if !body.contains_key("start") || !body.contains_key("end") {
            return Err(anyhow!(
                "start and end dateTimes are required to create an event"
            ));
        }

        let response = self
            .http
            .post(url)
            .bearer_auth(&token.access_token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<CalendarEvent>().await?)
    }

    pub async fn update_event(
        &self,
        token: &TokenInfo,
        event_id: &str,
        patch: &EventPayload,
    ) -> Result<CalendarEvent> {
        let calendar_id = self.resolve_calendar(&patch.calendar_id);
        let url = self.calendar_url(calendar_id, &format!("events/{event_id}"))?;

        let mut body = serde_json::to_value(patch)?
            .as_object()
            .cloned()
            .unwrap_or_default();
        body.remove("calendar_id");

        let response = self
            .http
            .patch(url)
            .bearer_auth(&token.access_token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<CalendarEvent>().await?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct ListEventsParams {
    pub calendar_id: Option<String>,
    pub time_min: Option<DateTime<Utc>>,
    pub time_max: Option<DateTime<Utc>>,
    pub max_results: Option<u32>,
    pub page_token: Option<String>,
    pub query: Option<String>,
    #[serde(default)]
    pub single_events: bool,
    #[serde(default)]
    pub order_by_start_time: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListEventsResponse {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub items: Vec<CalendarEvent>,
    #[serde(rename = "nextPageToken")]
    #[serde(default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct EventPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<EventDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<EventDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendees: Option<Vec<EventAttendee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminders: Option<EventReminders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conference_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Default, JsonSchema)]
pub struct EventDateTime {
    #[serde(rename = "dateTime", skip_serializing_if = "Option::is_none")]
    pub date_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}

impl<'de> Deserialize<'de> for EventDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            String(String),
            Object(EventDateTimeObject),
        }

        #[derive(Deserialize)]
        struct EventDateTimeObject {
            #[serde(rename = "dateTime")]
            date_time: Option<DateTime<Utc>>,
            #[serde(default)]
            time_zone: Option<String>,
        }

        let repr = Repr::deserialize(deserializer)?;
        match repr {
            Repr::String(value) => {
                let parsed = DateTime::parse_from_rfc3339(&value).map_err(|err| {
                    de::Error::custom(format!(
                        "failed to parse RFC3339 date-time string '{value}': {err}"
                    ))
                })?;
                Ok(EventDateTime {
                    date_time: Some(parsed.with_timezone(&Utc)),
                    time_zone: None,
                })
            }
            Repr::Object(object) => Ok(EventDateTime {
                date_time: object.date_time,
                time_zone: object.time_zone,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventAttendee {
    pub email: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(rename = "responseStatus", skip_serializing_if = "Option::is_none")]
    pub response_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventReminders {
    #[serde(default)]
    pub use_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<ReminderOverride>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReminderOverride {
    pub method: String,
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CalendarEvent {
    pub id: Option<String>,
    pub status: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: Option<EventDateTime>,
    pub end: Option<EventDateTime>,
    pub attendees: Option<Vec<EventAttendee>>,
    pub reminders: Option<EventReminders>,
    #[serde(rename = "htmlLink")]
    pub html_link: Option<String>,
    #[serde(rename = "created")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(rename = "updated")]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetEventParams {
    pub event_id: String,
    #[serde(default)]
    pub calendar_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serialization_strips_none() {
        let payload = EventPayload {
            summary: Some("Test".into()),
            ..Default::default()
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json.get("summary").unwrap().as_str().unwrap(), "Test");
        assert!(json.get("location").is_none());
    }

    #[test]
    fn event_date_time_accepts_rfc3339_string() {
        let json = "\"2025-10-14T12:34:56Z\"";
        let parsed: EventDateTime = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.date_time.unwrap().to_rfc3339(),
            "2025-10-14T12:34:56+00:00"
        );
        assert!(parsed.time_zone.is_none());
    }
}
