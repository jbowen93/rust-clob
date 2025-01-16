use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub side: OrderSide,
    #[serde(with = "ordered_float_serialize")]
    pub price: OrderedFloat<f64>,
    pub quantity: u64,
}

mod ordered_float_serialize {
    use ordered_float::OrderedFloat;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &OrderedFloat<f64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(value.0)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OrderedFloat<f64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Ok(OrderedFloat(value))
    }
}

#[derive(Clone)]
pub struct OrderBook {
    pub bids: BTreeMap<OrderedFloat<f64>, VecDeque<Order>>,
    pub asks: BTreeMap<OrderedFloat<f64>, VecDeque<Order>>,
}

impl OrderBook {
    pub fn new() -> Self {
        OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn submit_order(&mut self, mut order: Order) -> Vec<Order> {
        let mut trades = Vec::new();

        match order.side {
            OrderSide::Buy => {
                'matching: while order.quantity > 0 {
                    // Get the best ask price
                    let lowest_ask_price = match self.asks.keys().next() {
                        Some(price) => *price,
                        None => break 'matching,
                    };

                    if lowest_ask_price > order.price {
                        break 'matching;
                    }

                    // Process the orders at this price level
                    let queue = self.asks.get_mut(&lowest_ask_price).unwrap();
                    if let Some(mut ask_order) = queue.pop_front() {
                        let trade_qty = order.quantity.min(ask_order.quantity);
                        order.quantity -= trade_qty;
                        ask_order.quantity -= trade_qty;
                        trades.push(ask_order.clone());
                        
                        if ask_order.quantity > 0 {
                            queue.push_front(ask_order);
                        }
                    }

                    // Remove empty price levels
                    if queue.is_empty() {
                        self.asks.remove(&lowest_ask_price);
                    }
                }

                // Add remaining order to book
                if order.quantity > 0 {
                    self.bids
                        .entry(order.price)
                        .or_insert_with(VecDeque::new)
                        .push_back(order);
                }
            }
            OrderSide::Sell => {
                'matching: while order.quantity > 0 {
                    // Get the best bid price
                    let highest_bid_price = match self.bids.keys().next_back() {
                        Some(price) => *price,
                        None => break 'matching,
                    };

                    if highest_bid_price < order.price {
                        break 'matching;
                    }

                    // Process the orders at this price level
                    let queue = self.bids.get_mut(&highest_bid_price).unwrap();
                    if let Some(mut bid_order) = queue.pop_front() {
                        let trade_qty = order.quantity.min(bid_order.quantity);
                        order.quantity -= trade_qty;
                        bid_order.quantity -= trade_qty;
                        trades.push(bid_order.clone());
                        
                        if bid_order.quantity > 0 {
                            queue.push_front(bid_order);
                        }
                    }

                    // Remove empty price levels
                    if queue.is_empty() {
                        self.bids.remove(&highest_bid_price);
                    }
                }

                // Add remaining order to book
                if order.quantity > 0 {
                    self.asks
                        .entry(order.price)
                        .or_insert_with(VecDeque::new)
                        .push_back(order);
                }
            }
        }

        trades
    }

    pub fn cancel_order(&mut self, order_id: Uuid) -> bool {
        let mut found = false;
        for (_price, queue) in self.bids.iter_mut() {
            if let Some(pos) = queue.iter().position(|o| o.id == order_id) {
                queue.remove(pos);
                found = true;
                break;
            }
        }
        if !found {
            for (_price, queue) in self.asks.iter_mut() {
                if let Some(pos) = queue.iter().position(|o| o.id == order_id) {
                    queue.remove(pos);
                    found = true;
                    break;
                }
            }
        }
        found
    }

    pub fn get_order_book(&self) -> (Vec<Order>, Vec<Order>) {
        let mut bid_orders = Vec::new();
        for (_price, queue) in self.bids.iter().rev() {
            bid_orders.extend(queue.iter().cloned());
        }
        let mut ask_orders = Vec::new();
        for (_price, queue) in &self.asks {
            ask_orders.extend(queue.iter().cloned());
        }
        (bid_orders, ask_orders)
    }
}

struct AppState {
    order_book: Mutex<OrderBook>,
}

#[post("/submit")]
async fn submit_order(data: web::Data<AppState>, req_body: web::Json<Order>) -> impl Responder {
    let mut book = data.order_book.lock().unwrap();
    let mut order = req_body.into_inner();
    order.id = Uuid::new_v4();
    let trades = book.submit_order(order.clone());
    HttpResponse::Ok().json(trades)
}

#[post("/cancel")]
async fn cancel_order(data: web::Data<AppState>, req_body: web::Json<Uuid>) -> impl Responder {
    let mut book = data.order_book.lock().unwrap();
    let success = book.cancel_order(req_body.into_inner());
    if success {
        HttpResponse::Ok().body("Order canceled.")
    } else {
        HttpResponse::NotFound().body("Order not found.")
    }
}

#[get("/book")]
async fn get_book(data: web::Data<AppState>) -> impl Responder {
    let book = data.order_book.lock().unwrap();
    let (bids, asks) = book.get_order_book();
    let response = serde_json::json!({
        "bids": bids,
        "asks": asks,
    });
    HttpResponse::Ok().json(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let shared_state = web::Data::new(AppState {
        order_book: Mutex::new(OrderBook::new()),
    });

    println!("Starting CLOB server at http://localhost:8080/");
    HttpServer::new(move || {
        App::new()
            .app_data(shared_state.clone())
            .service(submit_order)
            .service(cancel_order)
            .service(get_book)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}