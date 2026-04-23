use matcher::book::{match_order, Book};
use matcher::types::{Order, Side};

fn o(id: u64, side: Side, price: u64, qty: u64) -> Order {
    Order { id, side, price, qty }
}

#[test]
fn empty_book_rests_order() {
    let mut book = Book::new();
    let fills = match_order(&mut book, o(1, Side::Buy, 100, 10));
    assert!(fills.is_empty());
    let snap = book.snapshot();
    assert_eq!(snap.bids.len(), 1);
    assert_eq!(snap.bids[0].price, 100);
    assert_eq!(snap.bids[0].qty, 10);
}

#[test]
fn exact_match_clears_both() {
    let mut book = Book::new();
    match_order(&mut book, o(1, Side::Sell, 100, 5));
    let fills = match_order(&mut book, o(2, Side::Buy, 100, 5));
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].maker_order_id, 1);
    assert_eq!(fills[0].taker_order_id, 2);
    assert_eq!(fills[0].price, 100);
    assert_eq!(fills[0].qty, 5);
    let snap = book.snapshot();
    assert!(snap.bids.is_empty());
    assert!(snap.asks.is_empty());
}

#[test]
fn partial_fill_rests_remainder() {
    let mut book = Book::new();
    match_order(&mut book, o(1, Side::Sell, 100, 3));
    let fills = match_order(&mut book, o(2, Side::Buy, 100, 10));
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].qty, 3);
    let snap = book.snapshot();
    assert!(snap.asks.is_empty());
    assert_eq!(snap.bids.len(), 1);
    assert_eq!(snap.bids[0].qty, 7);
}

#[test]
fn sweeps_multiple_price_levels_best_first() {
    let mut book = Book::new();
    match_order(&mut book, o(1, Side::Sell, 101, 4));
    match_order(&mut book, o(2, Side::Sell, 100, 3));
    let fills = match_order(&mut book, o(3, Side::Buy, 102, 10));
    assert_eq!(fills.len(), 2);
    assert_eq!(fills[0].price, 100);
    assert_eq!(fills[0].qty, 3);
    assert_eq!(fills[0].maker_order_id, 2);
    assert_eq!(fills[1].price, 101);
    assert_eq!(fills[1].qty, 4);
    assert_eq!(fills[1].maker_order_id, 1);
    let snap = book.snapshot();
    assert!(snap.asks.is_empty());
    assert_eq!(snap.bids[0].qty, 3);
    assert_eq!(snap.bids[0].price, 102);
}

#[test]
fn fifo_within_same_price_level() {
    let mut book = Book::new();
    match_order(&mut book, o(1, Side::Sell, 100, 5));
    match_order(&mut book, o(2, Side::Sell, 100, 5));
    let fills = match_order(&mut book, o(3, Side::Buy, 100, 7));
    assert_eq!(fills.len(), 2);
    assert_eq!(fills[0].maker_order_id, 1);
    assert_eq!(fills[0].qty, 5);
    assert_eq!(fills[1].maker_order_id, 2);
    assert_eq!(fills[1].qty, 2);
    let snap = book.snapshot();
    assert_eq!(snap.asks.len(), 1);
    assert_eq!(snap.asks[0].qty, 3);
}
