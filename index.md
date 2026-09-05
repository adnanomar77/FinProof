FinProof



Created by Adnan Omar Awad Allemon



Deterministic Financial Programming Language



FinProof is an independent domain-specific programming language for deterministic financial transactions, double-entry accounting, ledger execution, financial invariants, and execution verification.



Version



0.1.0



Language Pipeline



Source → Lexer → Parser → Type Checker → Compiler → Bytecode → VM → Ledger → Verification



Example



currency USD

account Cash: asset USD = 1000

account SalesRevenue: revenue USD = 0

transaction Sale {

&#x20;   debit Cash 100 USD

&#x20;   credit SalesRevenue 100 USD

}



Project



FinProof is an independent financial programming language and verification engine.

