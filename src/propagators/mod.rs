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

use std::fmt;

/// Provides different methods for controlling the error computation of the integrator.
pub mod error_ctrl;
pub use self::error_ctrl::*;

// Re-Export
mod instance;
pub use instance::*;
mod propagator;
pub use propagator::*;
mod rk_methods;
pub use rk_methods::*;
mod options;
pub use options::*;

use crate::time::Duration;

/// Stores the details of the previous integration step of a given propagator. Access as `my_prop.clone().latest_details()`.
#[derive(Copy, Clone, Debug)]
pub struct IntegrationDetails {
    /// step size used
    pub step: Duration,
    /// error in the previous integration step
    pub error: f64,
    /// number of attempts needed by an adaptive step size to be within the tolerance
    pub attempts: u8,
}

impl fmt::Display for IntegrationDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "IntegrationDetails {{step: {}, error: {:.3e}, attempts: {}}}",
            self.step, self.error, self.attempts
        )
    }
}
