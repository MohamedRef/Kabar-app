//! One username's Durable Object (`crate::login`): who owns it, and the
//! wrong-password count.

use std::cell::RefCell;

use worker::{DurableObject, Env, Method, Request, Response, Result, State, durable_object};

use super::{api_error, now_ms};
use crate::internal::{LoginCheck, LoginClear, LoginSet};
use crate::login::{Login as Store, UnknownTries, parse_key};
use kabar_protocol::cloud::ErrorCode;
use study_sql::durable::DoSql;

thread_local! {
    static UNKNOWN_TRIES: RefCell<UnknownTries> = RefCell::default();
}

#[durable_object]
pub struct Login {
    state: State,
}

fn sql_err(e: crate::sql::SqlError) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

impl DurableObject for Login {
    fn new(state: State, _env: Env) -> Self {
        Self { state }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let db = DoSql(self.state.storage().sql());
        let login = Store::new(&db).map_err(sql_err)?;
        let now = now_ms();
        match (req.method(), req.path().as_str()) {
            (Method::Post, "/check") => {
                let body: LoginCheck = req.json().await?;
                let Some(key) = parse_key(&body.key) else {
                    return api_error(ErrorCode::InvalidRequest, "password_key must be 32 bytes, base64url");
                };
                let checked = UNKNOWN_TRIES
                    .with_borrow_mut(|u| login.check_named(&body.username, &key, now, u))
                    .map_err(sql_err)?;
                Response::from_json(&checked)
            }
            (Method::Post, "/set") => {
                let b: LoginSet = req.json().await?;
                if login.set(&b.account_id, &b.email, &b.salt, &b.verifier, now).map_err(sql_err)? {
                    Response::empty()
                } else {
                    api_error(ErrorCode::UsernameTaken, "another account uses this username")
                }
            }
            (Method::Post, "/clear") => {
                let b: LoginClear = req.json().await?;
                login.clear(&b.account_id).map_err(sql_err)?;
                Response::empty()
            }
            _ => api_error(ErrorCode::NotFound, "not found"),
        }
    }
}
