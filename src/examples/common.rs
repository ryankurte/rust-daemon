/// Example request object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Get(String),
    Set(String, String),
}

/// Example response object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    None,
    Value(String),
}
