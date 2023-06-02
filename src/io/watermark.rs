/*
    Nyx, blazing fast astrodynamics
    Copyright (C) 2023 Christopher Rabotin <christopher.rabotin@gmail.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use std::collections::HashMap;

use hifitime::Epoch;
use parquet::{
    basic::{BrotliLevel, Compression},
    file::properties::WriterProperties,
    format::KeyValue,
};
use shadow_rs::shadow;
use whoami::{platform, realname, username};

shadow!(build);

/// The parquet writer properties
pub(crate) fn pq_writer(metadata: Option<HashMap<String, String>>) -> Option<WriterProperties> {
    let bldr = WriterProperties::builder()
        .set_compression(Compression::BROTLI(BrotliLevel::try_new(10).unwrap()));

    let mut file_metadata = vec![
        KeyValue::new(
            "Generated by".to_string(),
            format!("{} {}", build::PROJECT_NAME, build::PKG_VERSION),
        ),
        KeyValue::new(
            format!("{} License", build::PROJECT_NAME),
            "AGPL 3.0".to_string(),
        ),
        KeyValue::new(
            "Created by".to_string(),
            format!("{} ({}) on {}", realname(), username(), platform()),
        ),
        KeyValue::new(
            "Created on".to_string(),
            format!("{}", Epoch::now().unwrap()),
        ),
    ];

    if let Some(custom_md) = metadata {
        for (k, v) in custom_md {
            file_metadata.push(KeyValue::new(k, v));
        }
    }

    Some(bldr.set_key_value_metadata(Some(file_metadata)).build())
}
