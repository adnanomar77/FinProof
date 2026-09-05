currency USD
currency EUR

account CashUSD: asset USD = 0
account RevenueEUR: revenue EUR = 0

transaction WrongCurrencyBalance {
    debit CashUSD 100 USD
    credit RevenueEUR 100 EUR
}
