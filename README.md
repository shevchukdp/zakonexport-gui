# Zakon DB Exporter

Нативное Windows-приложение (Rust) для **поиска и экспорта документов из
базы Закон** (`C:\Zakon.Local\Baza`, `C:\Zakon.Local\Baza.GK`): прямое
чтение файлов базы (`.db`/`.mb`) — не требует установки и запуска каких-либо
сторонних компонентов, работает без сети. База читается **только для чтения**.

Два исполняемых файла из одного движка:

| Бинарник | Назначение |
|---|---|
| `zakonexport-gui.exe` | двойной клик → окно (без консоли в release) |
| `zakonexport-cli.exe` | все команды: search / export / bulk / print / … + `gui` (запуск GUI) |

## Возможности

- **Поиск** по всему тексту базы: токенизированный формат `.mb` разворачивается
  нативно; параллельный обход чанков (rayon); регистронезависимый,
  кириллически-безопасный (Unicode Win32 UI, cp1251-консоль не мешает —
  запрос можно взять из файла `--queryfile`).
- **Две базы**: `--base general` (Zakon → `C:\Zakon.Local\Baza`),
  `--base audit` (ГК → `C:\Zakon.Local\Baza.GK`), `--baza <path>` — переопределение.
  Перемещённая установка (другой путь): GUI спрашивает папку и запоминает в
  `%APPDATA%\zakonexport\zakonexport.toml`, CLI-команды используют тот же путь.
- **Экспорт** с сохранением форматирования (капс-жирный, маркеры «•  »,
  центрированные строки): `md`, `rtf`, `docx` (valid zip + CRC-32), `html`,
  `csv` (таблица результатов одним файлом) и **`pdf`** — чистый Rust-райтер:
  PDF 1.4 со встраиваемым **сабсетом системного шрифта**
  `C:\Windows\Fonts\arial.ttf` (cmap/glyf/hmtx/loca/post, FLATE-поток,
  /Differences + /Widths). Никакого внешнего PDF-генератора.
- **GDI-друк** (без Word и других приложений): системный диалог печати
  (`PrintDlg`), GDI-цикл страниц, отмена → статус «відмінено», не ошибка.
  CLI: `print <docnum>`; в GUI — кнопка «Друк» в окне предпросмотра.
- **Фильтры по реквизитам**: `--status actual|canceled|notpub|all`,
  `--issuer`, `--type`, `--date-from/--date-to`, `--title`.
- **По номеру документа**: `--number` (в т.ч. «з № 249», «до 249»),
  точный выбор `--docs 136974,222-225` (работает и без запроса),
  интерактивный `--pick`.
- **Приложения (signal)**: `--attach` копирует `<baza>\signal\<docnum>.<ext>`
  к экспорту и делает пометку в отчёте; команда `signal <docnum>` —
  локализовать приложение документа.
- **GUI** (native-windows-gui): поиск в рабочем потоке (UI не блокируется),
  предпросмотр документа, выбор формата из списка, COM folder picker для
  пути экспорта, самотест `--guitest` (пишет `gui_ready.txt`).
- **Чистый Rust**: BZip2 (libbz2-rs), inflate (miniz_oxide) — ни одной C-зависимости,
  сборка без C-компилятора в PATH.

## Сборка

Требуется rustup с GNU-тулчейном (self-contained mingw, C-компилятор не нужен):

```
rustup default stable-x86_64-pc-windows-gnu
cargo build --release
```

Результат: `target\release\zakonexport-cli.exe`, `target\release\zakonexport-gui.exe`
(без иконки). Иконку: `powershell -File res\inject_icon.ps1`
(+ проверка `res\extract_icon.ps1` / `make_icon.ps1`).

Тесты:

```
cargo test --lib       # юнит-тесты движка, writers, pdf (51 тест)
zakonexport-gui.exe --guitest --baza C:\Zakon.Local\Baza.GK   # GUI-самотест
```

## CLI

```
zakonexport-cli search <query> [--and|--or|--phrase] [--number] [--max n] [--fields]
zakonexport-cli export <query> [фильтры] [--format md|rtf|docx|html|csv|pdf] [--out dir]
                       [--docs n1,n2-n5 | --pick] [--attach]
zakonexport-cli export --docs 136974,222-225 [--format pdf] [--out dir]
zakonexport-cli fields <docnum>      # реквизиты документа
zakonexport-cli extract <docnum>     # полный текст документа
zakonexport-cli signal <docnum>      # найти signal-приложение (если есть)
zakonexport-cli print  <docnum>     # GDI-друк (системный диалог печати)
zakonexport-cli bulk [--format …] [--out dir] [--max n]
zakonexport-cli stats
zakonexport-cli about               # версия/автор
zakonexport-cli gui [--guitest] [--base general|audit|<path>] [--baza <path>]
```

Полезные флаги поиска:

```
--queryfile <utf8.txt>   # запрос из файла (обходит cp1251-консоль)
--list <utf8.txt>        # весь пронумерованный список результатов (с реквизитами) в файл
--out <dir>              # папка экспорта (по умолчанию C:\temp\opencode\export)
```

## Структура

```
src\main.rs        CLI-команды (все verb'ы)
src\bin\zakonexport-gui.rs  точка входа GUI (без консоли)
src\lib.rs         версия, about, persistence пути базы
src\db.rs          .db/.mb: запись/span'ы, cp1251+fold-кодек, поисковый движок
src\render.rs      развёртка токенов в текст, титулы/превью, выбор базы
src\writers.rs     md/rtf/docx/html/csv + диспетч export_hits()
src\pdf.rs         TTF-парсер/сабсетирование + PDF 1.4-сборка (FontFile2)
src\printdoc.rs    GDI-печать: PrintDlg + цикл страниц, Win32 FFI вручную
src\gui.rs         native-windows-gui UI (рабочий поток, предпросмотр, «Друк»)
src\picker.rs      COM IFileDialog folder picker
src\naming.rs      шаблоны/санитизация/дедупликация имён, run-папки
src\fields.rs      парсер реквизитов; src\mdconv.rs, src\signal.rs, src\update.rs
res\               иконки + inject_icon.ps1 (реинжект иконки после release)
build\run-gui.bat  запуск GUI двойным кликом
```

## Известные особенности

- **Шрифт PDF**: встраиваемый сабсет `arial.ttf` берётся из системы. На части
  машин системный `arial.ttf` имеет «усталый» cmap: часть кириллических кодов
  указывает за пределы собственной glyph-таблицы шрифта. Райтер закрывает
  такие маппинги в `.notdef` (gid → 0), т.е. символы, которых у шрифта
  объективно нет, рисуются как пустые глифы. Для машин с обычным Arial
  (full Cyrillic) это ограничение не проявляется.
- База читается **только для чтения**; каталог по умолчанию
  `C:\temp\opencode\export`.

## Релізи / Releases

Історія версій — в **CHANGELOG.md**: кожен запис UA + EN, сторінки
GitHub-релізу несуть ті самі нотатки (UA + EN).
Version history — in **CHANGELOG.md**: every entry UK + EN, the GitHub
release pages carry the same notes (UK + EN).

Поточний / Current:
- **v0.10.0** — GDI-друк (системний діалог, `print <docnum>`, кнопка «Друк»),
  PDF-експорт (чистий Rust, Arial-сабсет), 51 юніт-тест. / GDI printing
  (system dialog), pure-Rust PDF export (Arial subset), 51 unit tests.
- **v0.9.0** — ядро: read-only `.db`/`.mb`, пошук (rayon), експорт
  md/rtf/docx/html/csv, bulk, фільтри, GUI, реквізити/extract/signal;
  бинарники з іконкою. / Core: read-only `.db`/`.mb`, search (rayon),
  md/rtf/docx/html/csv export, bulk, filters, GUI, fields/extract/signal;
  icon-injected binaries.
- **v0.8.0 / v0.7.0 / v0.6.0 / v0.4.0** — архівні GUI-zip (pre-git) /
  archived GUI zips (pre-git era).
- **v0.1–v0.3** — ранні ітерації, бинарників немає / early iterations,
  no archived binaries.

Процес релізу / Release process: `CHANGELOG.md` (UA+EN) → тег + сторінка
релізу (UA+EN) → `releases/<v>/` в репо + ассети
(`.github/workflows/release-assets.yml`; завантаження йде через
`uploads.github.com`). / `CHANGELOG.md` (UA+EN) → tag + release page
(UA+EN) → `releases/<v>/` in-repo + assets (uploads go through
`uploads.github.com`).
