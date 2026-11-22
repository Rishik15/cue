//! File: platform/os_values.rs
//! Purpose: The two things from the operating system that a snippet's placeholders read, so the template itself stays pure: an environment variable and random numbers.
//! Contents: env_var — one variable's text; random_u64 — a number from the system generator.

/// The value of the environment variable `name`; None when it is not set or not text. Never logged.
pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// A random number from the system generator; None when the generator failed, so a caller never makes up a value.
pub fn random_u64() -> Option<u64> {
    getrandom::u64().ok()
}
