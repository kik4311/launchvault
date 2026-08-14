<div align="center">

# LaunchVault

**Персональный игровой центр для Linux и Windows**

Вся твоя библиотека в одном месте — запуск, обложки, статистика и Steam-игры без облака и лишних аккаунтов.

[![Rust](https://img.shields.io/badge/Rust-1.97-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![egui](https://img.shields.io/badge/egui-0.36-blueviolet)](https://github.com/emilk/egui)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Windows-blue?logo=linux&logoColor=white)](https://github.com/kik4311/launchvault)
[![License](https://img.shields.io/badge/License-GPL--3.0-green)](LICENSE)

---

</div>

## Возможности

| | |
|---|---|
| **Локальная библиотека** | Игры в одной сетке: название, обложка, время, история запусков |
| **Поиск** | Быстрый фильтр по названию прямо из главного экрана |
| **Таймер сессий** | Время игры считается автоматически — останови и оно сохранится |
| **Обложки** | Свои картинки или автозагрузка из Steam CDN |
| **Steam-синхронизация** | Сканирует установленные игры, запуск через `steam://`, обложки — всё локально |
| **Настройки** | Тёмная/светлая тема, путь к Steam, очистка кэша |
| **Приватность** | Никаких облаков и аккаунтов — данные только на твоём устройстве |

> **Покупка игр удалена намеренно** — LaunchVault служит для запуска и просмотра, покупки совершаются только в самом Steam.

---

## Установка

### Сборка из исходников

Требуется [Rust](https://rustup.rs) и системные зависимости WebKitGTK для Linux.

```bash
# Linux: системные зависимости (Debian/Ubuntu/Fedora)
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev build-essential
# или: sudo dnf install webkit2gtk4.1-devel gtk3-devel

git clone https://github.com/kik4311/launchvault.git
cd launchvault

cargo build --release       # оптимизированная сборка
./target/release/launchvault
```

### Windows

```powershell
git clone https://github.com/kik4311/launchvault.git
cd launchvault
cargo build --release
.\target\release\launchvault.exe
```

---

## Использование

1. **Библиотека** — нажми «+ Игра», укажи название и путь к запуску. Обложку можно добавить файлом или URL.
2. **Steam** — нажми «Синхронизировать»: установленные игры появятся списком. Кнопка «+ Добавить» перенесёт игру в библиотеку с обложкой.
3. **Таймер** — при запуске игры сессия стартует автоматически. Кнопка «Стоп» (или выход) сохранит время.

---

## Структура

```
src/
├── main.rs        # точка входа, окно
├── app.rs         # интерфейс (вкладки, карточки, диалоги)
├── models.rs      # модели данных, пути хранения
├── storage.rs     # сохранение/загрузка JSON
├── steam.rs       # Steam: скан, обложки, запуск, таймер
└── vdf.rs         # парсер VDF (appmanifest / libraryfolders)
```

---

## Данные

Всё хранится локально:

- **Linux:** `~/.local/share/launchvault/`
- **Windows:** `%APPDATA%\launchvault\`

Для переноса на другой ПК просто скопируй папку целиком.

---

## Технологии

- **Rust** — производительность и надёжность
- **egui / eframe** — мгновенный native-интерфейс без веб-фронтенда
- **serde / serde_json** — данные
- **ureq** — загрузка обложек
- **VDF-парсер** — чтение файлов Steam без лишних зависимостей

---

## Лицензия

[GPL-3.0](LICENSE) — свободное ПО с копилефтом: можно использовать, копировать и модифицировать, производные работы обязаны распространяться под той же лицензией.

**© 2026 kik4311**
