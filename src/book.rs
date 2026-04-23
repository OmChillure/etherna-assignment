use std::collections::{BTreeMap, VecDeque};

use crate::types::{Fill, Level, Order, Side, Snapshot};

#[derive(Debug, Default)]
pub struct Book {
    pub bids: BTreeMap<u64, VecDeque<Order>>,
    pub asks: BTreeMap<u64, VecDeque<Order>>,
}

impl Book {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Snapshot {
        let bids = self
            .bids
            .iter()
            .rev()
            .map(|(&price, level)| Level {
                price,
                qty: level.iter().map(|o| o.qty).sum(),
            })
            .collect();
        let asks = self
            .asks
            .iter()
            .map(|(&price, level)| Level {
                price,
                qty: level.iter().map(|o| o.qty).sum(),
            })
            .collect();
        Snapshot { bids, asks }
    }

    fn rest(&mut self, order: Order) {
        let side = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        side.entry(order.price).or_default().push_back(order);
    }
}

pub fn match_order(book: &mut Book, mut taker: Order) -> Vec<Fill> {
    let mut fills = Vec::new();

    loop {
        if taker.qty == 0 {
            break;
        }

        let best_price = match taker.side {
            Side::Buy => book.asks.keys().next().copied(),
            Side::Sell => book.bids.keys().next_back().copied(),
        };
        let Some(price) = best_price else { break };

        let crosses = match taker.side {
            Side::Buy => taker.price >= price,
            Side::Sell => taker.price <= price,
        };
        if !crosses {
            break;
        }

        let opposite = match taker.side {
            Side::Buy => &mut book.asks,
            Side::Sell => &mut book.bids,
        };
        let level = opposite.get_mut(&price).expect("level exists");

        while taker.qty > 0 {
            let Some(maker) = level.front_mut() else { break };
            let qty = taker.qty.min(maker.qty);

            fills.push(Fill {
                maker_order_id: maker.id,
                taker_order_id: taker.id,
                price,
                qty,
            });

            maker.qty -= qty;
            taker.qty -= qty;
            if maker.qty == 0 {
                level.pop_front();
            }
        }

        if level.is_empty() {
            opposite.remove(&price);
        }
    }

    if taker.qty > 0 {
        book.rest(taker);
    }
    fills
}
