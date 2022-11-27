use serde::Deserialize;

#[derive(Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub gender: String,
    pub phone: String,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct Users {
    pub users: Vec<User>
}
