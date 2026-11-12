//! The full screens as egui widgets in the page frame ([`crate::page`]), one module each. A
//! screen takes the `Ui` the window gives it and returns the keys its clicks stand for, which the
//! window feeds the shell as if typed (docs/design/egui-frontend.md, "Mouse"); the rows and the
//! words are `fn_ui`'s.

pub mod build;
pub mod research;
