#!/bin/bash

# Define the API endpoint
API="http://localhost:8080"

# Function to print response with a label
print_response() {
    echo ">>> $1"
    echo "$2" | jq '.' || echo "$2"
    echo
}

# Function to get current order book state
get_book() {
    echo "Current Order Book:"
    curl -s "$API/book" | jq '.'
    echo
}

# Start with checking the empty order book
get_book

# Place a few sell orders at different price levels
echo "Placing sell orders..."
SELL1=$(curl -s -X POST "$API/submit" \
    -H "Content-Type: application/json" \
    -d '{"id":"00000000-0000-0000-0000-000000000000","side":"Sell","price":100.50,"quantity":10}')
print_response "Sell order 1 (100.50)" "$SELL1"

SELL2=$(curl -s -X POST "$API/submit" \
    -H "Content-Type: application/json" \
    -d '{"id":"00000000-0000-0000-0000-000000000000","side":"Sell","price":101.00,"quantity":5}')
print_response "Sell order 2 (101.00)" "$SELL2"

get_book

# Place some buy orders that shouldn't match
echo "Placing buy orders below sell price..."
BUY1=$(curl -s -X POST "$API/submit" \
    -H "Content-Type: application/json" \
    -d '{"id":"00000000-0000-0000-0000-000000000000","side":"Buy","price":99.00,"quantity":7}')
print_response "Buy order 1 (99.00)" "$BUY1"

get_book

# Place a matching buy order
echo "Placing matching buy order..."
BUY2=$(curl -s -X POST "$API/submit" \
    -H "Content-Type: application/json" \
    -d '{"id":"00000000-0000-0000-0000-000000000000","side":"Buy","price":101.00,"quantity":8}')
print_response "Buy order 2 (101.00) - should match" "$BUY2"

get_book

# Extract an order ID from the order book to test cancellation
ORDER_ID=$(curl -s "$API/book" | jq -r '.bids[0].id // empty')

if [ ! -z "$ORDER_ID" ]; then
    echo "Cancelling order $ORDER_ID..."
    CANCEL=$(curl -s -X POST "$API/cancel" \
        -H "Content-Type: application/json" \
        -d "\"$ORDER_ID\"")
    echo "Cancel response: $CANCEL"
    
    get_book
else
    echo "No orders available to cancel"
fi