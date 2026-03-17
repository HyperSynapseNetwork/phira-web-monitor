use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VisitedUserInfo {
    pub phira_id: i32,
}

#[derive(Debug, Serialize)]
pub struct VisitedUserListResponse {
    pub count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<VisitedUserInfo>>,
}
