use std::fmt::Display;

pub fn verbose_result_ok<T, E: Display>(context: String, result: Result<T, E>) -> Option<T> {
    match result {
        Ok(t) => Some(t),
        Err(e) => {
            warn!("{}\nError: {}", context, e);
            None
        }
    }
}