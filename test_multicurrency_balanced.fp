currency USD
currency EUR

account CashUSD: asset USD = 100
account RevenueUSD: revenue USD = 0
account CashEUR: asset EUR = 100
account RevenueEUR: revenue EUR = 0

transaction MultiCurrency {
    debit CashUSD 40 USD
    credit RevenueUSD 40 USD
    debit CashEUR 30 EUR
    credit RevenueEUR 30 EUR
}
