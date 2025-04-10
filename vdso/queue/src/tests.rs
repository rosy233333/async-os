use super::*;
/// Copied from: https://github.com/ClSlaid/l3queue/blob/466f507186cd342e8eb886e79d209b7606460b30/src/he_queue.rs#L166-L333
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

#[no_mangle]
fn get_data_base() -> usize {
    0
}

#[test]
fn test_single() {
    let q = LockFreeQueue::new();
    q.push(1);
    q.push(1);
    q.push(4);
    q.push(5);
    q.push(1);
    q.push(4);
    assert_eq!(q.pop(), Some(1));
    assert_eq!(q.pop(), Some(1));
    assert_eq!(q.pop(), Some(4));
    assert_eq!(q.pop(), Some(5));
    assert_eq!(q.pop(), Some(1));
    assert_eq!(q.pop(), Some(4));
}

#[test]
fn test_concurrent_send() {
    let pad = 100000_u128;

    let p1 = Arc::new(LockFreeQueue::new());
    let p2 = p1.clone();
    let c = p1.clone();
    let ba1 = Arc::new(Barrier::new(3));
    let ba2 = ba1.clone();
    let ba3 = ba1.clone();
    let t1 = thread::spawn(move || {
        for i in 0..pad {
            p1.push(i);
        }
        ba1.wait();
    });
    let t2 = thread::spawn(move || {
        for i in pad..(2 * pad) {
            p2.push(i);
        }
        ba2.wait();
    });
    // receive after send is finished
    ba3.wait();
    let mut sum = 0;
    while let Some(got) = c.pop() {
        sum += got;
    }
    let _ = t1.join();
    let _ = t2.join();
    assert_eq!(sum, (0..(2 * pad)).sum())
}

#[test]
fn test_mpsc() {
    let pad = 100_0000u128;

    let flag = Arc::new(AtomicI32::new(3));
    let flag1 = flag.clone();
    let flag2 = flag.clone();
    let flag3 = flag.clone();
    let p1 = Arc::new(LockFreeQueue::new());
    let p2 = p1.clone();
    let p3 = p1.clone();
    let c = p1.clone();

    let t1 = thread::spawn(move || {
        for i in 0..pad {
            p1.push(i);
        }
        flag1.fetch_sub(1, Ordering::SeqCst);
    });
    let t2 = thread::spawn(move || {
        for i in pad..(2 * pad) {
            p2.push(i);
        }
        flag2.fetch_sub(1, Ordering::SeqCst);
    });
    let t3 = thread::spawn(move || {
        for i in (2 * pad)..(3 * pad) {
            p3.push(i);
        }
        flag3.fetch_sub(1, Ordering::SeqCst);
    });

    let mut sum = 0;
    while flag.load(Ordering::SeqCst) != 0 || !c.is_empty() {
        if let Some(num) = c.pop() {
            sum += num;
        }
    }

    t1.join().unwrap();
    t2.join().unwrap();
    t3.join().unwrap();
    assert_eq!(sum, (0..(3 * pad)).sum());
}

#[test]
fn test_mpmc() {
    let pad = 10_0000u128;

    let flag = Arc::new(AtomicI32::new(3));
    let flag_c = flag.clone();
    let flag1 = flag.clone();
    let flag2 = flag.clone();
    let flag3 = flag.clone();

    let p1 = Arc::new(LockFreeQueue::new());
    let p2 = p1.clone();
    let p3 = p1.clone();
    let c1 = p1.clone();
    let c2 = p1.clone();

    let producer1 = thread::spawn(move || {
        for i in 0..pad {
            p1.push(i);
        }
        flag1.fetch_sub(1, Ordering::SeqCst);
    });
    let producer2 = thread::spawn(move || {
        for i in pad..(2 * pad) {
            p2.push(i);
        }
        flag2.fetch_sub(1, Ordering::SeqCst);
    });
    let producer3 = thread::spawn(move || {
        for i in (2 * pad)..(3 * pad) {
            p3.push(i);
        }
        flag3.fetch_sub(1, Ordering::SeqCst);
    });

    let consumer = thread::spawn(move || {
        let mut sum = 0;
        while flag_c.load(Ordering::SeqCst) != 0 || !c2.is_empty() {
            if let Some(num) = c2.pop() {
                sum += num;
            }
        }
        sum
    });

    let mut sum = 0;
    while flag.load(Ordering::SeqCst) != 0 || !c1.is_empty() {
        if let Some(num) = c1.pop() {
            sum += num;
        }
    }

    producer1.join().unwrap();
    producer2.join().unwrap();
    producer3.join().unwrap();

    let s = consumer.join().unwrap();
    sum += s;
    assert_eq!(sum, (0..(3 * pad)).sum());
}
