//! Reference: [https://www.lumissil.com/assets/pdf/core/IS31FL3194_DS.pdf]

pub mod controller;
mod model;
pub mod patterns;
#[cfg(test)]
mod tests;

pub use controller::IS31FL3194Controller;
pub use patterns::LedPattern;
