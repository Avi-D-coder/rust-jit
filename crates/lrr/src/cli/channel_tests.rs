//! `std::sync::mpsc` over the single-threaded facade queue. Expected output is real rustc
//! 1.99.0's. Each program runs in all four editions. Sends append and never block,
//! `recv` takes the oldest message, and dropping every sender (or the receiver)
//! disconnects the channel.
use super::format_trait_facade_tests::prints;

/// FIFO order, `Sender::clone`, `try_recv` Empty/Disconnected, `recv` after every sender is
/// dropped, `sync_channel`'s `try_send` bound and a send to a dropped receiver.
#[test]
fn channels_deliver_in_order_and_report_disconnection() {
    prints(
        "channel-basics",
        "use std::sync::mpsc::{channel, sync_channel};
fn main() {
    let (tx, rx) = channel::<i32>();
    let tx2 = tx.clone();
    tx.send(1).unwrap();
    tx2.send(2).unwrap();
    println!(\"{:?} {:?}\", rx.recv(), rx.try_recv());
    println!(\"{:?}\", rx.try_recv());
    drop(tx);
    drop(tx2);
    println!(\"{:?} {:?}\", rx.try_recv(), rx.recv());
    let (s, r) = sync_channel::<u8>(1);
    s.send(5).unwrap();
    println!(\"{} {:?}\", s.try_send(6).is_err(), r.recv());
    drop(r);
    println!(\"{}\", s.send(7).is_err());
}
",
        "Ok(1) Ok(2)\nErr(Empty)\nErr(Disconnected) Err(RecvError)\ntrue Ok(5)\ntrue\n",
    );
}

/// A spawned closure runs to completion before `spawn` returns, so a producer thread's
/// messages are queued when the receiver reads them.
#[test]
fn spawned_producer_runs_before_the_receiver() {
    prints(
        "channel-spawn",
        "use std::sync::mpsc::channel;
use std::thread;
fn main() {
    let (tx, rx) = channel::<i32>();
    let h = thread::spawn(move || {
        for i in 0..5 { tx.send(i).unwrap(); }
        42i32
    });
    println!(\"{}\", h.join().unwrap());
    let mut sum = 0;
    while let Ok(v) = rx.recv() { sum += v; }
    println!(\"{}\", sum);
}
",
        "42\n10\n",
    );
}
