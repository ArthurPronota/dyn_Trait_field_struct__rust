trait Logger {
    fn log(&self, msg: &str) ;
}

struct App {
    logger: Box<dyn Logger + Send + Sync>
}

struct Console ;

impl Logger for Console {
    fn log(&self, msg: &str) {
        println!("{}", msg) ;
    }
}

fn main() {
    let app = App{
        logger: Box::new(Console)
    } ;

    app.logger.log("abc");  // Out: abc
}
