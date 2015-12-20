use log;

pub fn init() -> Result<(), log::SetLoggerError> {
    log::set_logger(|max_level| {
        max_level.set(log::LogLevelFilter::Debug);
        Box::new(ConsoleLogger)
    })
}

struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= log::LogLevel::Debug
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
}
