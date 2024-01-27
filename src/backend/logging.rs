use chrono::{DateTime, Local};
use owo_colors::{AnsiColors, OwoColorize};
use std::io::{stdout, IsTerminal};
use std::{env, fmt};
use tracing_core::{Event, Level, LevelFilter, Subscriber};
use tracing_subscriber::{
    field::MakeExt,
    fmt::{
        format::{self, format, FormatEvent, FormatFields},
        FmtContext,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Less noisy formatter for tracing-subscriber
pub struct PrettyFormatter {
    timer: DateTime<Local>,
}

impl Default for PrettyFormatter {
    fn default() -> Self {
        Self {
            timer: Local::now(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for PrettyFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();

        let timestamp = if writer.has_ansi_escapes() {
            self.timer
                .format("%H:%M:%S")
                .color(AnsiColors::BrightBlack)
                .to_string()
        } else {
            self.timer.format("%H:%M:%S").to_string()
        };

        write!(writer, "{timestamp} ")?;

        let (level_icon, level_style) = match *metadata.level() {
            Level::TRACE => ('…', AnsiColors::Magenta),
            Level::DEBUG => (' ', AnsiColors::White),
            Level::INFO => ('ℹ', AnsiColors::Blue),
            Level::WARN => ('⚠', AnsiColors::BrightYellow),
            Level::ERROR => ('✖', AnsiColors::Red),
        };

        let icon = if writer.has_ansi_escapes() {
            level_icon.color(level_style).to_string()
        } else {
            level_icon.to_string()
        };

        write!(writer, "{icon} ")?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

// Set up tracing-subscriber
pub fn setup() {
    // default to INFO for release builds, DEBUG otherwise
    const LEVEL: LevelFilter = if cfg!(debug_assertions) {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    // Newline-separated fields
    let field_fmt = tracing_subscriber::fmt::format::debug_fn(|writer, field, value| {
        write!(
            writer,
            "{}{:?}",
            if field.name() == "message" {
                String::new()
            } else {
                format!("{field}: ")
            },
            value
        )
    })
    .delimited("\n\t · ");

    let output = match env::var("ELEANOR_VERBOSE") {
        Ok(_) => tracing_subscriber::fmt::layer()
            .with_ansi(stdout().is_terminal())
            .event_format(format())
            .boxed(),
        // `ELEANOR_VERBOSE` is not set, default to pretty logs
        Err(_) => tracing_subscriber::fmt::layer()
            .with_ansi(stdout().is_terminal())
            .event_format(PrettyFormatter::default())
            .fmt_fields(field_fmt)
            .boxed(),
    };

    let log = tracing_subscriber::registry().with(output);

    if env::var("RUST_LOG").is_ok_and(|v| !v.is_empty()) {
        log.with(EnvFilter::from_default_env()).init();
    } else {
        log.with(LEVEL).init();
    }
}
