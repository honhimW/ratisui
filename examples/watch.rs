use tokio::join;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, mut rx) = watch::channel(false);

    let tx_clone = tx.clone();
    let mut rx_clone = rx.clone();
    let handle1 = tokio::spawn(async move {
        assert!(!rx_clone.has_changed().unwrap());
    });
    handle1.await?;
    let handle2 = tokio::spawn(async move {
        tx_clone.send(true).unwrap();
    });
    handle2.await?;
    println!("in main: {:?}", rx.has_changed()?);
    let mut rx_clone = rx.clone();
    let handle3 = tokio::spawn(async move {
        assert!(rx_clone.has_changed().unwrap());
        println!("in handle3: {:?}", *rx_clone.borrow());
        assert_eq!(*rx_clone.borrow(), true);
    });
    handle3.await?;

    Ok(())
}