# async_quotation_simulation

Стриминг искусственных котировок: **TCP** для команд (`STREAM`, `PING`/`PONG`), **UDP** для потока котировок и keep-alive (`PING`/`PONG` по UDP).

## Структура проекта

| Компонент | Описание |
|-----------|----------|
| `src/lib.rs` | Корень библиотеки `utils`: протокол, модель, сеть, клиент и сервер как модули |
| `src/bin/server.rs` | Бинарник `server` |
| `src/bin/client.rs` | Бинарник `client` |
| `src/protocol.rs` | Текстовый протокол команд и ответов `OK` / `ERR` |
| `src/model.rs` | `StockQuote`, сериализация в JSON для UDP |
| `tickers.txt` | Список тикеров, по которым генерирует сервер |

## Сборка

```bash
cargo build
```

Собираются цели `server` и `client` (нужен feature `cli` по умолчанию).

## Запуск сервера

```bash
RUST_LOG=info cargo run --bin server -- --listen 0.0.0.0:9876 --emit-interval-ms 25
```

В **stdout** печатается одна строка `READY <addr>` — по ней можно взять адрес для клиента (в т.ч. при `--listen 127.0.0.1:0`). Остальные служебные сообщения идут через `log` / `env_logger` (stderr).

## Запуск клиента

Подписка на поток (файл тикеров — по одному тикеру в строке, пустые строки игнорируются):

```bash
cargo run --bin client -- stream \
  --server-addr 127.0.0.1:9876 \
  --udp-port 34254 \
  --tickers-file tickers_mini.txt \
  --bind 127.0.0.1
```

Проверка TCP-канала (`PING` / `PONG` по TCP):

```bash
cargo run --bin client -- tcp-ping --server-addr 127.0.0.1:9876
```

### Команда STREAM (клиент → сервер по TCP)

```text
STREAM udp://<ip>:<port> TICKER1,TICKER2,...
```

Ответ сервера одной строкой:

- `OK` — подписка принята, начинается UDP-стрим;
- `ERR <сообщение>` — неверные параметры (пустой список тикеров, дубликаты, неизвестный тикер и т.д.).

### UDP

- Котировки: одна датаграмма = одна строка JSON  
  `{"ticker":"AAPL","price":...,"volume":...,"timestamp":...}`.
- Keep-alive: клиент шлёт `PING` на адрес источника датаграмм с котировками; сервер отвечает `PONG`. При отсутствии `PING` дольше **5 с** стрим для этого клиента останавливается.

## Тесты

```bash
cargo test
```
