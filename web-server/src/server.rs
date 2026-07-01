// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Axum router + listener wiring. Kept thin on purpose — all request
//! handling lives in `handlers.rs`.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::get;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::database::Database;
use crate::handlers::{self, AppState};

pub async fn run(config: Config, db: Database, smoke: bool) -> Result<()> {
    let bind_addr: SocketAddr = format!("{}:{}", config.bind_ip(), config.port())
        .parse()
        .with_context(|| {
            format!(
                "parsing bind address {}:{}",
                config.bind_ip(),
                config.port()
            )
        })?;

    let state = AppState {
        db: Arc::new(db),
        session_hours: config.session_hours(),
    };

    let app = Router::new()
        .route("/", get(handlers::root))
        .route(
            "/login",
            get(handlers::login_form).post(handlers::login_submit),
        )
        .route(
            "/signup",
            get(handlers::signup_form).post(handlers::signup_submit),
        )
        .route("/healthz", get(handlers::healthz))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(%bind_addr, "web server listening");
    let listener = match tokio::net::TcpListener::bind(bind_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            if smoke {
                std::process::exit(common::smoke::smoke_fail(
                    "Web",
                    "startup",
                    &e.to_string(),
                    common::smoke::EXIT_STARTUP,
                ));
            }
            return Err(e).with_context(|| format!("binding {bind_addr}"));
        }
    };

    if smoke {
        let _ = common::smoke::smoke_ok("Web", &bind_addr.to_string());
        return Ok(());
    }

    axum::serve(listener, app.into_make_service())
        .await
        .context("axum::serve")?;
    Ok(())
}
