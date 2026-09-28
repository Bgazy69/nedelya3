use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

/// Упрощённая модель Solana-аккаунта.
///
/// В реальной Solana каждый account хранит адрес (публичный ключ), owner-программу
/// и произвольные данные. Здесь для простоты — только адрес и баланс в lamports
/// (1 SOL = 1_000_000_000 lamports).
#[derive(Debug, Clone)]
struct Account {
    address: String,
    balance: u64,
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (баланс: {} lamports)", self.address, self.balance)
    }
}

/// Инструкция, которую может обработать наша "программа".
///
/// В реальной Solana `Instruction` — это тоже enum: разные варианты того,
/// что может "попросить сделать" транзакция (Transfer, CreateAccount и т.д.),
/// и program определяет, как каждый вариант обрабатывать.
#[derive(Debug)]
enum Instruction {
    CreateAccount { address: String, initial_balance: u64 },
    Transfer { from: String, to: String, amount: u64 },
}

/// Собственный тип ошибки — аналог `ProgramError` в настоящих Solana Programs:
/// программа никогда не паникует, а возвращает понятную типизированную ошибку.
#[derive(Debug, PartialEq)]
enum ProgramError {
    AccountNotFound(String),
    InsufficientFunds { address: String, needed: u64, available: u64 },
    AccountAlreadyExists(String),
}

// Реализация трейта Display — чтобы ошибку можно было красиво напечатать
impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProgramError::AccountNotFound(addr) => {
                write!(f, "Аккаунт не найден: {}", addr)
            }
            ProgramError::InsufficientFunds { address, needed, available } => {
                write!(f, "Недостаточно средств на {}: нужно {}, доступно {}", address, needed, available)
            }
            ProgramError::AccountAlreadyExists(addr) => {
                write!(f, "Аккаунт уже существует: {}", addr)
            }
        }
    }
}

/// "Программа" хранит состояние всех аккаунтов — как runtime Solana хранит
/// все accounts в сети. Это упрощённый аналог on-chain state.
/// Дополнительно хранит журнал транзакций (`history`) для команды "Transactions".
struct Program {
    accounts: HashMap<String, Account>,
    history: Vec<String>,
}

impl Program {
    /// Создаёт программу с пустым набором аккаунтов.
    fn new() -> Self {
        Program { accounts: HashMap::new(), history: Vec::new() }
    }

    /// Главный паттерн Solana-программ: `process_instruction`.
    /// Возвращает `Result<(), ProgramError>` — либо успех, либо конкретная ошибка,
    /// вместо паники — точно как это работает в реальном Solana runtime.
    fn process_instruction(&mut self, ix: Instruction) -> Result<(), ProgramError> {
        match ix {
            Instruction::CreateAccount { address, initial_balance } => {
                if self.accounts.contains_key(&address) {
                    return Err(ProgramError::AccountAlreadyExists(address));
                }
                if initial_balance > 0 {
                    self.history.push(format!("{} получил airdrop {} lamports", address, initial_balance));
                }
                self.accounts.insert(
                    address.clone(),
                    Account { address, balance: initial_balance },
                );
                Ok(())
            }

            Instruction::Transfer { from, to, amount } => {
                let from_balance = self.get_balance(&from)?; // ? — пробрасывает ошибку выше, если есть
                if from_balance < amount {
                    return Err(ProgramError::InsufficientFunds {
                        address: from,
                        needed: amount,
                        available: from_balance,
                    });
                }
                self.accounts.get_mut(&from).unwrap().balance -= amount;
                self.accounts
                    .entry(to.clone())
                    .or_insert(Account { address: to.clone(), balance: 0 })
                    .balance += amount;
                self.history.push(format!("{} -> {}: {} lamports", from, to, amount));
                Ok(())
            }
        }
    }

    /// Возвращает баланс аккаунта либо ошибку `AccountNotFound`.
    /// Использует `Option` через `.map()`/`.ok_or_else()` — стандартный паттерн Rust.
    fn get_balance(&self, address: &str) -> Result<u64, ProgramError> {
        self.accounts
            .get(address)
            .map(|acc| acc.balance)
            .ok_or_else(|| ProgramError::AccountNotFound(address.to_string()))
    }
}

/// Читает строку, введённую пользователем, и убирает лишние пробелы/перенос строки.
fn read_line() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Не удалось прочитать ввод");
    input.trim().to_string()
}

fn main() {
    let mut program = Program::new();

    // Стартовое состояние: один кошелёк с airdrop 5 SOL (5_000_000_000 lamports),
    // как в примере с настоящим Devnet-кошельком из недели 1.
    const WALLET: &str = "Devnet Wallet";
    program
        .process_instruction(Instruction::CreateAccount {
            address: WALLET.to_string(),
            initial_balance: 5_000_000_000,
        })
        .unwrap();

    loop {
        println!("\n=== Solana CLI ===");
        println!("1. Wallet");
        println!("2. Balance");
        println!("3. Transactions");
        println!("4. Exit");
        print!("Choose an option: ");
        io::stdout().flush().unwrap();

        let choice = read_line();

        match choice.as_str() {
            "1" => {
                println!("Wallet address: {}", WALLET);
            }
            "2" => {
                // Result здесь тоже обрабатывается через match, а не unwrap() —
                // даже если баланс не найдётся, программа не упадёт.
                match program.get_balance(WALLET) {
                    Ok(balance) => println!("Balance: {} lamports (~{:.3} SOL)", balance, balance as f64 / 1_000_000_000.0),
                    Err(e) => println!("Ошибка: {}", e),
                }
            }
            "3" => {
                println!("Хочешь отправить перевод? (y/n)");
                if read_line().eq_ignore_ascii_case("y") {
                    print!("Адрес получателя: ");
                    io::stdout().flush().unwrap();
                    let to = read_line();

                    print!("Сумма в lamports: ");
                    io::stdout().flush().unwrap();
                    let amount_input = read_line();

                    // parse() возвращает Result — обрабатываем некорректный ввод пользователя
                    match amount_input.parse::<u64>() {
                        Ok(amount) => {
                            match program.process_instruction(Instruction::Transfer {
                                from: WALLET.to_string(),
                                to,
                                amount,
                            }) {
                                Ok(()) => println!("Перевод выполнен."),
                                Err(e) => println!("Ошибка: {}", e),
                            }
                        }
                        Err(_) => println!("Некорректная сумма, введи целое число."),
                    }
                }

                println!("Transactions:");
                if program.history.is_empty() {
                    println!("  (пусто)");
                } else {
                    for entry in &program.history {
                        println!("  - {}", entry);
                    }
                }
            }
            "4" => {
                println!("Goodbye!");
                break;
            }
            _ => println!("Неизвестная опция, попробуй снова."),
        }
    }
}

// === ТЕСТЫ ===
// В Rust тесты пишутся прямо в этом же файле, в модуле с атрибутом #[cfg(test)] —
// компилятор включает этот код только при запуске `cargo test`, в обычной сборке
// (`cargo run`) он отсутствует.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_account_works() {
        let mut program = Program::new();
        let result = program.process_instruction(Instruction::CreateAccount {
            address: "Alice".to_string(),
            initial_balance: 1000,
        });
        assert!(result.is_ok());
        assert_eq!(program.get_balance("Alice").unwrap(), 1000);
    }

    #[test]
    fn cannot_create_duplicate_account() {
        let mut program = Program::new();
        program.process_instruction(Instruction::CreateAccount {
            address: "Alice".to_string(),
            initial_balance: 1000,
        }).unwrap();

        let result = program.process_instruction(Instruction::CreateAccount {
            address: "Alice".to_string(),
            initial_balance: 500,
        });

        assert_eq!(result, Err(ProgramError::AccountAlreadyExists("Alice".to_string())));
    }

    #[test]
    fn transfer_moves_balance_correctly() {
        let mut program = Program::new();
        program.process_instruction(Instruction::CreateAccount {
            address: "Alice".to_string(),
            initial_balance: 1000,
        }).unwrap();
        program.process_instruction(Instruction::CreateAccount {
            address: "Bob".to_string(),
            initial_balance: 0,
        }).unwrap();

        program.process_instruction(Instruction::Transfer {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            amount: 300,
        }).unwrap();

        assert_eq!(program.get_balance("Alice").unwrap(), 700);
        assert_eq!(program.get_balance("Bob").unwrap(), 300);
    }

    #[test]
    fn transfer_fails_on_insufficient_funds() {
        let mut program = Program::new();
        program.process_instruction(Instruction::CreateAccount {
            address: "Alice".to_string(),
            initial_balance: 100,
        }).unwrap();
        program.process_instruction(Instruction::CreateAccount {
            address: "Bob".to_string(),
            initial_balance: 0,
        }).unwrap();

        let result = program.process_instruction(Instruction::Transfer {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            amount: 5000,
        });

        assert_eq!(
            result,
            Err(ProgramError::InsufficientFunds {
                address: "Alice".to_string(),
                needed: 5000,
                available: 100,
            })
        );
        // Баланс не должен был измениться после неудачного перевода
        assert_eq!(program.get_balance("Alice").unwrap(), 100);
    }

    #[test]
    fn transfer_to_unknown_account_creates_it() {
        let mut program = Program::new();
        program.process_instruction(Instruction::CreateAccount {
            address: "Alice".to_string(),
            initial_balance: 1000,
        }).unwrap();

        // Bob ещё не создан явно — System Program в реальной Solana
        // тоже может создать аккаунт получателя "на лету"
        program.process_instruction(Instruction::Transfer {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            amount: 200,
        }).unwrap();

        assert_eq!(program.get_balance("Bob").unwrap(), 200);
    }

    #[test]
    fn balance_check_on_missing_account_fails() {
        let program = Program::new();
        let result = program.get_balance("Ghost");
        assert_eq!(result, Err(ProgramError::AccountNotFound("Ghost".to_string())));
    }
}