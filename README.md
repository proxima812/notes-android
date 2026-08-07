# xima.keeps — локальный Android-органайзер

Заметки, поиск по ним и системные напоминания. Работает полностью на устройстве:
без сервера, аккаунта, облачной синхронизации, аналитики и обязательного интернета.

## Скачать готовое приложение

Подписанный APK каждого выпуска лежит на странице релизов:

- **Последняя версия:** <https://github.com/proxima812/notes-android/releases/latest>
  (файл `ximakeeps-X.Y.Z-arm64.apk`)
- Все выпуски и что в них менялось: [Releases](https://github.com/proxima812/notes-android/releases)
  и [CHANGELOG.md](CHANGELOG.md)

Установка: скачать APK на телефон, открыть его и разрешить установку из этого
источника. Обновление ставится поверх — данные сохраняются (подпись у всех
релизов одна).

### Версионирование

Версии идут по [SemVer](https://semver.org/lang/ru/) — `МАЖОРНАЯ.МИНОРНАЯ.ПАТЧ`
(несовместимые изменения / новые возможности / исправления). Номер версии
одинаков в `package.json`, `src-tauri/tauri.conf.json` и `src-tauri/Cargo.toml`;
каждому релизу соответствует git-тег `vX.Y.Z`, запись в
[CHANGELOG.md](CHANGELOG.md) и APK в Releases. Установленная версия видна в
Android: **Настройки → Приложения → xima.keeps**.

- **Оболочка:** Tauri 2
- **Ядро:** Rust (бизнес-логика, SQLite, поиск, валидация)
- **Интерфейс:** React 19 + TypeScript + Tailwind CSS v4
- **Хранилище:** локальная SQLite + FTS5
- **Напоминания:** Rust + Kotlin (`AlarmManager`, `NotificationManager`)
- **Цель:** подписанный APK для Google Pixel 8a (`arm64-v8a`)

## 0. Что уже работает, а чего ещё нет

Схема БД (`migrations/0001_initial.sql`) заложена под весь продукт целиком —
задачи, вложения, теги, повторения, бэкапы. Написана из этого пока меньшая
часть, и таблица ниже — про написанное, а не про схему. Папки схема тоже несла;
их убрали целиком — миграцией `0002_drop_folders.sql`, — потому что теги делают
то же самое одним способом вместо двух.

| Есть | Ещё нет |
| --- | --- |
| Заметки: rich text, шаблоны, цветовые градиенты | Вложения и голосовые заметки |
| Корзина: удаление, восстановление, очистка | Шифрование базы |
| Полнотекстовый поиск (FTS5) + история запросов | Приоритеты и сроки у задач |
| Несколько напоминаний на заметку, со звуком и шаблонами времени | |
| Повторы: ежедневно, по будням, еженедельно, ежемесячно, ежегодно | |
| «Отложить» кнопкой прямо в уведомлении | |
| Пересчёт напоминаний при смене часового пояса | |
| Восстановление будильников после перезагрузки | |
| Резервная копия и восстановление из файла | |
| Чек-листы: задачи как строки, а не как галочки в тексте | |
| Теги: фильтр в библиотеке и строка под карточкой | |
| Темы оформления (по умолчанию — как в системе) и 8 языков интерфейса | Смена иконки приложения (отложена, см. docs/plans) |

Если чего-то нет в левой колонке, значит этого не написали — сколько бы места
под это ни было заложено в схеме базы.

---

## 1. Требования к машине сборки

Проверенные версии (macOS arm64):

| Компонент | Версия | Где взять |
| --- | --- | --- |
| Rust | 1.97.1 stable | `rustup` |
| Rust target | `aarch64-linux-android` | `rustup target add aarch64-linux-android` |
| Node | 22.x | — |
| Bun | 1.3.x | пакетный менеджер проекта |
| JDK | **21** | `brew install openjdk@21` |
| Android SDK Platform | `android-36` | `sdkmanager` |
| Android Build-Tools | `36.1.0` | `sdkmanager` |
| Android NDK | `27.3.13750724` | `sdkmanager` |
| Platform-Tools (`adb`) | последняя | `sdkmanager` |

> **JDK 26 не подходит.** Android Gradle Plugin поддерживает JDK 17/21;
> на 26 сборка падает. Именно поэтому `JAVA_HOME` ниже указывает на `openjdk@21`.

### Установка SDK с нуля

```bash
brew install openjdk@21
brew install --cask android-commandlinetools   # даёт sdkmanager
brew install android-platform-tools            # даёт adb

export ANDROID_HOME="$HOME/Library/Android/sdk"
mkdir -p "$ANDROID_HOME"
yes | sdkmanager --sdk_root="$ANDROID_HOME" --licenses
sdkmanager --sdk_root="$ANDROID_HOME" \
  "platform-tools" "platforms;android-36" "build-tools;36.1.0" "ndk;27.3.13750724"
```

## 2. Переменные окружения

Обязательны для любой Android-команды. Используйте готовый скрипт — он ещё и
проверяет, что тулчейн на месте:

```bash
source ./scripts/android-env.sh
java -version        # должно быть 21.x
cargo --version
adb devices
```

Что именно он выставляет:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export JAVA_HOME=/opt/homebrew/opt/openjdk@21
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/27.3.13750724"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
```

## 3. Команды

```bash
bun install                  # зависимости фронтенда

bun run dev                  # только Vite (браузер, без Rust-команд)
bun run build                # tsc -b + vite build

bun run check:ts             # строгий TypeScript
bun run check:rust           # cargo check
bun run lint:rust            # clippy с -D warnings
bun run fmt:rust             # cargo fmt
bun run test                 # vitest
bun run test:rust            # cargo test

bun run android:init         # однократная генерация gen/android
bun run android:dev          # запуск на подключённом устройстве с HMR
bun run android:build:debug  # debug APK (arm64)
bun run android:build        # release APK (arm64)
```

Полная проверка перед сдачей этапа:

```bash
source ./scripts/android-env.sh
bun run check:ts
bun run build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/plugins/reminders/Cargo.toml
cargo test --manifest-path src-tauri/plugins/documents/Cargo.toml
cargo test --manifest-path src-tauri/plugins/appicon/Cargo.toml
bun run test
bun run test:kotlin
bun run android:build
```

### Непрерывная интеграция

`.github/workflows/ci.yml` гоняет на каждый push всё, что можно проверить без
устройства: TypeScript, vitest, сборку фронтенда, `cargo fmt`, clippy и тесты
ядра с плагинами.

Kotlin-тесты туда не входят: модуль плагина собирается против `.tauri/tauri-api`,
который генерируется сборкой Tauri и в чистом клоне отсутствует. Их запускает
`bun run test:kotlin` после хотя бы одной локальной Android-сборки.

## 4. Установка на Pixel 8a

1. На телефоне: **Настройки → О телефоне → Номер сборки** — нажать 7 раз.
2. **Настройки → Система → Для разработчиков → Отладка по USB** — включить.
3. Подключить кабелем, подтвердить отпечаток ключа на экране телефона.
4. Проверить: `adb devices` — устройство должно быть `device`, а не `unauthorized`.

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
```

Живая разработка с горячей перезагрузкой:

```bash
bun run android:dev
```

Логи ядра:

```bash
adb logcat -s RustStdoutStderr:D Organizer:D
```

## 5. Подпись release APK

Keystore и пароли **никогда не попадают в Git** — они уже в `.gitignore`.

### 5.1 Создание keystore

Выполнить один раз и **сохранить файл и пароли в надёжном месте**. Потеря
keystore означает невозможность выпустить обновление поверх установленного APK.

Имя файла — `приложение-разработчик-дата`, чтобы по нему было видно, каким
ключом подписан установленный на телефоне APK.

```bash
mkdir -p "$HOME/.android-keystores"
chmod 700 "$HOME/.android-keystores"

source ./scripts/android-env.sh
keytool -genkeypair -v \
  -keystore "$HOME/.android-keystores/ximakeeps-proxima812-20260802.jks" \
  -alias ximakeeps \
  -keyalg RSA \
  -keysize 4096 \
  -validity 10000 \
  -storetype PKCS12
```

`keytool` спросит пароль хранилища и данные владельца (CN, организация, страна).
Для PKCS12 пароль ключа совпадает с паролем хранилища.

### 5.2 Подключение keystore к сборке

Создайте `src-tauri/gen/android/keystore.properties` (игнорируется Git):

```properties
storeFile=/Users/ВАШ_ПОЛЬЗОВАТЕЛЬ/.android-keystores/ximakeeps-proxima812-20260802.jks
storePassword=ВАШ_ПАРОЛЬ
keyAlias=ximakeeps
keyPassword=ВАШ_ПАРОЛЬ
```

```bash
chmod 600 src-tauri/gen/android/keystore.properties
```

Конфигурация подписи активна только когда этот файл существует; иначе
release-сборка останавливается с понятным сообщением вместо выпуска
неподписанного APK.

### 5.3 Сборка и проверка подписи

```bash
bun run android:build

APK=src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release.apk
"$ANDROID_HOME/build-tools/36.1.0/apksigner" verify --print-certs "$APK"
adb install -r "$APK"
```

> Если на устройстве уже стоит debug-сборка, release с другой подписью поверх не
> установится. Сначала `adb uninstall dev.local.organizer`.

## 6. Структура

```text
src/                  React UI (только интерфейс)
├── app/              провайдеры, роутинг, тема
├── pages/            LibraryPage, NoteEditorPage
├── features/         notes, reminders, tasks, organisation, backup,
│                     appearance, settings
├── shared/           api, i18n, lib, test, types, ui
└── styles/           Tailwind v4

src-tauri/
├── src/
│   ├── domain/          notes, reminders, tasks, organisation, backup,
│   │                    app_icons, settings, search, clock, ids
│   ├── application/     commands, use_cases, dto
│   ├── infrastructure/  sqlite (connection, migrations, репозитории)
│   ├── platform/        мост к Android-плагинам
│   ├── error.rs
│   ├── state.rs
│   └── lib.rs
├── migrations/          версионируемые SQL-миграции
├── capabilities/        разрешения Tauri
├── plugins/reminders/   Tauri-плагин: Rust API + Kotlin (AlarmManager)
├── plugins/documents/   Tauri-плагин: системный выбор файла (копии)
├── plugins/appicon/     Tauri-плагин: переключение иконки лаунчера
└── gen/android/         Gradle-проект (генерируется)
```

Правила, которые не нарушаются:

- React не обращается к SQLite и не содержит бизнес-логики;
- Kotlin содержит только вызовы Android API, без бизнес-логики;
- SQL живёт в repositories, не в Tauri-командах;
- системные напоминания не используют `setTimeout`/`setInterval`;
- Kotlin помнит только те будильники, которые сам поставил (`AlarmStore`), —
  это не вторая копия напоминаний, а то, что `BootReceiver` заново отдаёт
  системе после перезагрузки, не заглядывая в базу.

## 7. Приватность

Приложение не имеет сетевого кода. CSP в `tauri.conf.json` запрещает любые
внешние источники (`default-src 'self'`), список разрешений Tauri в
`src-tauri/capabilities/default.json` намеренно минимален.
