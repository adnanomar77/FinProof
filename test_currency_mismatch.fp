currency USD
currency EUR

account Alice: asset USD = 100
account Bob: asset EUR = 0

transaction CrossCurrency {
    pay 50 USD
    from Alice
    to Bob
}
