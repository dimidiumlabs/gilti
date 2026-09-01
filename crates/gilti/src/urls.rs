// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub fn repository(repository: &str) -> String {
    format!("/{}", encode_path(repository))
}

pub fn encode_path(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'/' || byte == b'_' {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02x}").expect("writing to String cannot fail");
        }
    }
    encoded
}
