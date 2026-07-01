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

//! `--smoke` boot support: fixed exit codes plus single-line `SMOKE_OK` /
//! `SMOKE_FAIL` markers that CI and `scripts/smoke-local.sh` grep for.
//!
//! When a service is launched with `--smoke` it boots only far enough to prove
//! its config parses, its database opens, and (for the map server) its content
//! catalogs load, then prints one marker line via [`smoke_ok`] and exits `0`
//! instead of entering its accept loop. Staged failures short-circuit through
//! [`smoke_fail`] with the matching exit code below.

/// Every staged smoke check passed.
pub const EXIT_OK: i32 = 0;
/// Config file was missing, unreadable, or failed to parse.
pub const EXIT_CONFIG: i32 = 10;
/// Database failed to open or ping.
pub const EXIT_DATABASE: i32 = 20;
/// A startup step failed (listener bind, content load, etc.).
pub const EXIT_STARTUP: i32 = 30;
/// A step failed after startup completed.
pub const EXIT_RUNTIME: i32 = 40;
/// An otherwise-unclassified failure.
pub const EXIT_UNHANDLED: i32 = 50;

/// Print the success marker for `server` bound at `endpoint` and return
/// [`EXIT_OK`]. Callers typically `return Ok(())` right afterwards so the
/// process exits cleanly with code `0`.
pub fn smoke_ok(server: &str, endpoint: &str) -> i32 {
    println!("SMOKE_OK {server} {endpoint}");
    EXIT_OK
}

/// Print the failure marker for `server` in `category` with a sanitized `msg`,
/// returning `code` so the caller can `std::process::exit(code)`.
pub fn smoke_fail(server: &str, category: &str, msg: &str, code: i32) -> i32 {
    println!("SMOKE_FAIL {server} {category}: {}", sanitize(msg));
    code
}

/// Collapse any run of carriage-returns / newlines into a single space and trim
/// the ends, so a multi-line error renders as one grep-friendly line.
fn sanitize(msg: &str) -> String {
    msg.split(['\r', '\n'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_collapses_newline_runs_into_single_spaces() {
        // Arrange / Act / Assert: a CRLF run and a bare-LF run each collapse
        // to exactly one space.
        assert_eq!(sanitize("line one\r\n\r\nline two"), "line one line two");
        assert_eq!(sanitize("a\nb\nc"), "a b c");
    }

    #[test]
    fn sanitize_trims_surrounding_breaks_and_whitespace() {
        assert_eq!(sanitize("\n  hello world  \n"), "hello world");
        assert_eq!(sanitize(""), "");
    }

    #[test]
    fn smoke_ok_returns_exit_ok_zero() {
        assert_eq!(smoke_ok("World", "127.0.0.1:54992"), EXIT_OK);
        assert_eq!(EXIT_OK, 0);
    }

    #[test]
    fn smoke_fail_returns_the_given_code() {
        assert_eq!(
            smoke_fail("Map", "database", "boom\nnext", EXIT_DATABASE),
            EXIT_DATABASE
        );
    }
}
