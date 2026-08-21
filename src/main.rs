#[tokio::main]
async fn main() {
    if std::env::args()
        .skip(1)
        .any(|a| a == "--version" || a == "-V")
    {
        println!("stars {}", stars::VERSION);
        return;
    }
    stars::run().await;
}
