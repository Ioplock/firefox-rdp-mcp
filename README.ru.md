# firefox-rdp-mcp

[English](README.md)

**MCP-сервер на Rust с транспортом stdio** для Firefox, которым совместно
пользуются человек и ИИ-агент. Подключается к уже запущенному серверу Firefox
DevTools Remote Debugging Protocol (RDP). Сам Firefox не запускает, расширения
не устанавливает, WebDriver не использует.

Nix-пакет включает закреплённый [ff-rdp](https://github.com/ractive/ff-rdp)
с патчем выбора вкладок по `bc:ID`. Создание вкладок и загрузка адресов выполняются
через браузерный RDP-контекст, чтение страниц и взаимодействие — через ff-rdp.

## Сборка и запуск

Нужен Nix с включёнными `nix-command` и `flakes`. Предусмотрены outputs для
`x86_64-linux` и `aarch64-linux`; проверки выполнены на x86_64 NixOS.
В окружении разработки только инструменты сборки/тестирования и backend —
**без Firefox и ИИ-клиента**. Первая сборка может скачивать зависимости.

```sh
git clone git@github.com:Ioplock/firefox-rdp-mcp.git
cd firefox-rdp-mcp
nix build --cores 2 --max-jobs 1
# Сервер для MCP-клиента, а не интерактивная консоль:
./result/bin/firefox-rdp-mcp
```

`nix run .` запускает тот же stdio-сервер. stdout предназначен исключительно
для JSON-RPC, диагностика выводится в stderr. Сервер ожидает initialize от клиента.

## Подключение в flake другого проекта

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    firefox-rdp-mcp.url = "github:Ioplock/firefox-rdp-mcp";
  };
  outputs = { nixpkgs, firefox-rdp-mcp, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      devShells.${system}.default = pkgs.mkShell {
        packages = [ firefox-rdp-mcp.packages.${system}.default ];
      };
    };
}
```

Запускайте MCP-клиент из `nix develop` проекта-потребителя. Сохраните его
`flake.lock` в Git, чтобы закрепить ревизию MCP. Для использования совместимой
версии nixpkgs потребителя можно дополнительно указать
`inputs.firefox-rdp-mcp.inputs.nixpkgs.follows = "nixpkgs"`.
Для приватного репозитория вместо GitHub-сокращения используйте
`git+ssh://git@github.com/Ioplock/firefox-rdp-mcp.git`.

Также доступен overlay:

```nix
pkgs = import nixpkgs {
  inherit system;
  overlays = [ firefox-rdp-mcp.overlays.default ];
};
# pkgs.firefox-rdp-mcp и pkgs.ff-rdp
```

## Настройка MCP-клиента

Пример для клиентов с форматом `mcpServers`. Сам клиент должен быть запущен
из dev shell проекта-потребителя:

```json
{
  "mcpServers": {
    "firefox": {
      "command": "firefox-rdp-mcp",
      "env": { "FF_RDP_HOST": "127.0.0.1", "FF_RDP_PORT": "6000" }
    }
  }
}
```

OpenCode использует другой формат локальной конфигурации:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "firefox": {
      "type": "local",
      "command": ["firefox-rdp-mcp"],
      "enabled": true,
      "timeout": 130000,
      "environment": { "FF_RDP_HOST": "127.0.0.1", "FF_RDP_PORT": "6000" }
    }
  }
}
```

GUI-клиенту, запущенному вне shell, нужен абсолютный путь к установленному
исполняемому файлу либо команда наподобие
`["nix", "run", "--cores", "2", "--max-jobs", "1", "github:Ioplock/firefox-rdp-mcp", "--"]`.
Соберите пакет заранее, чтобы сборка не превысила таймаут подключения клиента.
После обновления переподключите MCP: уже запущенный сервер сохраняет старую версию.

## Требования к Firefox и настройка

Нужен **настольный Firefox с DevTools RDP и отладкой браузерного chrome-контекста**.
Текущая реализация проверена на **Firefox 157.0 под Linux**; другие версии и
форки Firefox пока не проверялись. Firefox намеренно не входит в зависимости
сборки MCP. Если общий настроенный браузер уже запущен, используйте его —
дополнительный экземпляр запускать не нужно.

Для **нового отдельного профиля** создайте каталог вне дерева исходников:

```sh
FIREFOX_PROFILE="${XDG_STATE_HOME:-$HOME/.local/state}/firefox-rdp-mcp/profile"
mkdir -p "$FIREFOX_PROFILE"
```

Создайте в нём `user.js` с указанными настройками. Для существующего профиля
объедините их с имеющимися настройками, не перезаписывая файл целиком.

```js
user_pref("devtools.debugger.remote-enabled", true);
user_pref("devtools.chrome.enabled", true);
user_pref("devtools.debugger.force-local", true);
user_pref("devtools.debugger.prompt-connection", false);
```

Запустите выделенный браузер вручную:

```sh
firefox --no-remote --profile "$FIREFOX_PROFILE" --start-debugger-server 6000 about:blank
```

`devtools.chrome.enabled` нужен для создания вкладок и навигации. Оставьте
`force-local` включённым: RDP даёт управление браузером без аутентификации.
Под Linux проверьте `ss -ltn 'sport = :6000'`: слушать должен только loopback.
Нужен `--start-debugger-server`, а не `--remote-debugging-port` — это другой
протокол. Изменение настроек требует перезапуска именно этого Firefox.

Не включайте профиль, cookies и сессии ни в Git, ни в исходники, передаваемые
Nix. Для локального path flake без Git даже игнорируемые runtime-файлы могут
попасть в Nix store. Профиль вне каталога проекта предотвращает эту проблему.

Документация Firefox: [параметры запуска](https://firefox-source-docs.mozilla.org/browser/CommandLineParameters.html),
[настройки Browser Toolbox](https://firefox-source-docs.mozilla.org/devtools-user/browser_toolbox/index.html),
[протокол RDP](https://firefox-source-docs.mozilla.org/devtools/backend/protocol.html).

## Инструменты и поведение

26 инструментов охватывают статус/вкладки, snapshots, текст, навигацию,
клики/ввод/прокрутку/историю, JavaScript, консоль/сеть, скриншоты,
хранилища/cookies, стили/исходники, производительность, ожидание готовности
и обнаружение/извлечение списков.

- Начинайте с `firefox_status`. Используйте устойчивые `bc:ID` из status/list.
  Поиск по URL/заголовку должен быть однозначным. Выбор страницы меняет цель
  MCP, но не фокусирует вкладку в GUI. Закрытая цель требует нового выбора.
- Для действий предпочитайте refs из snapshot. Пользователь может изменить
  страницу в любой момент — после этого обновляйте snapshot. CAPTCHA/MFA
  пользователь проходит вручную.
- `firefox_navigate` принимает `new_tab`, `wait` (`interactive` по умолчанию,
  также `complete` и `loading`) и `wait_selector`. Через 10 секунд этап ожидания
  может вернуть `partial`: используйте `firefox_wait` или проверьте страницу,
  не повторяя уже начатое действие.
- Инструменты выполняются последовательно. Общий `timeout_ms` включает очередь:
  по умолчанию 25000, допустимый диапазон 100–120000 мс. Отмена/таймаут
  завершают процесс backend. Действие могло уже произойти — не повторяйте вслепую.
- Структурированный JSON ограничен 16000 байт с отметкой об опущенных данных;
  изображения возвращаются отдельно. Сохранившиеся URL и refs не обрезаются.
  Для больших страниц используйте query, `firefox_discover_lists` и пагинацию
  `firefox_extract`: `offset`, `next_offset`, `has_more`. Параметр `max` поля
  ограничивает текст/атрибуты; href сохраняет полный рабочий URL.

### Удобство для агентов

- `firefox_close_page` требует явный `{"tab":"bc:58"}`. Закрывает только эту
  вкладку и возвращает `closed`, оставшиеся `tabs` и цель MCP `selected`.
  `closed:false` означает, что закрытие не подтверждено (например, страница
  запросила подтверждение); перед повтором проверьте состояние. При закрытии
  текущей цели выбирается оставшаяся. Закрывайте только разрешённые пользователем вкладки.
- `firefox_find_elements`: `role`, обязательный `name`, `exact` (по умолчанию
  true; false — подстрока без учёта регистра). Возвращает все совпадения с refs,
  `total`, `offset`, `next_offset`, `has_more`, `omitted`. Передавайте ref в
  click/type; при нескольких совпадениях не выбирайте первое вслепую.
  Роли/имена определяются по DOM и не заменяют полную платформенную доступность.
  Скрытые/inert элементы исключаются; disabled отображается. Поиск ограничен
  основным документом; для frames/shadow DOM может потребоваться дерево backend
  или дополнительное исследование.
- `firefox_snapshot` по умолчанию использует `format:"compact"`: плоский список
  интерактивных элементов, диалоги и основное содержимое раньше шапки/футера.
  Доступны `query`, `offset`, `limit`, `max_chars`. `format:"tree"` сохраняет
  прежнее дерево backend и параметр `depth`. Имена длиннее 500 символов имеют
  отметку `name_truncated`; поиск сравнивает полное имя. URL/refs не обрезаются.
  Встроенные страницы navigate и click/type также компактные. Клиентам прежнего
  вложенного формата следует отдельно запрашивать `format:"tree"`.
- Готовность навигации: `status:ready|pending|target_missing|read_failed`.
  Потеря цели сопровождается списком вкладок, когда он доступен; выберите
  актуальный `bc:ID`. При `pending` используйте `firefox_wait`, а не reload или
  повтор навигации. Потеря цели до действия — ошибка аргумента с данными для
  восстановления. Вкладка с похожим URL не подставляется автоматически.
- Перед click/type, прокруткой, извлечением и ожиданием проверяется стандартный
  CSS. Ошибочный `button:contains(...)` возвращает ошибку аргумента вместо
  вводящего в заблуждение таймаута. Используйте поиск по роли/имени.
- `firefox_discover_lists` возвращает `extract_example` с селекторами полей,
  готовый для `firefox_extract`. `price_candidate` — эвристика: проверьте
  единицы, скидки и наличие по извлечённому тексту.
- Click/type принимают `expect`: `selector` (по умолчанию body), `state`
  (`appeared`, `disappeared`, `text_contains`, `text_changed`), а для
  `text_contains` ещё `text`. Появление означает наличие в DOM. Для полей
  наблюдается value, для остальных элементов — текст. `text_changed`
  сравнивает со значением до действия. `postcondition.confirmed` отражает
  наблюдаемое условие отдельно от выполнения действия. Невыполненное условие
  даёт `pending`; ошибка чтения и потеря цели указываются отдельно. Действие
  без условия не подтверждает результат в приложении.
- `firefox_wait` принимает такое же `condition`; для `text_changed` требуется
  `previous_text`. Условия опрашиваются до пяти секунд в рамках общего
  дедлайна. Если подтверждения нет, проверьте состояние или продолжите ожидание;
  не повторяйте изменение вслепую. Перед заявлением об успехе сверьте фактическую
  корзину/форму/приложение. Одно совпадение текста не доказывает покупку или отправку.

```json
{"role":"button","name":"В корзину","exact":true}
```

Передайте это в `firefox_find_elements`, затем используйте однозначный ref:

```json
{"ref":"e583016","expect":{"selector":"#cart-count","state":"text_changed"}}
```

| Переменная окружения | По умолчанию | Назначение |
|---|---|---|
| `FF_RDP_HOST` | `127.0.0.1` | Адрес уже запущенного Firefox RDP |
| `FF_RDP_PORT` | `6000` | TCP-порт RDP |
| `FF_RDP_TIMEOUT_MS` | `8000` | Максимальное время одного процесса backend |
| `FF_RDP_BIN` | `ff-rdp` | Backend для cargo/необёрнутого бинарника; Nix-пакет принудительно использует свой patched backend |
| `RUST_LOG` | логирование info | Фильтр tracing в stderr |

Если Firefox недоступен, инструменты возвращают ошибку, а не запускают его.
Версия `0.2.0` в status подтверждает подключение этой версии MCP.

## Разработка и проверки

```sh
nix develop
cargo fmt --check
cargo test --locked -j 2
nix flake check --cores 2 --max-jobs 1
```

`flake check` запускает Rust-тесты, включая завершение процесса при таймауте,
проверку форматирования, автономные unit-тесты скриптов в Node.js и проверку протокола/схем MCP.
Браузер не нужен. `tests/mcp_client.py` использует только стандартную библиотеку Python.

Для явной проверки с уже работающим общим Firefox:

```sh
nix build --cores 2 --max-jobs 1
nix develop --command python3 tests/browser_smoke.py ./result/bin/firefox-rdp-mcp
```

Тест создаёт вкладки с локальной HTTP-страницей, проверяет навигацию/готовность,
извлечение, одинаковые URL/заголовки, действия, таймауты и отмену. Закрывает
только собственные вкладки и проверяет сохранность исходных идентификаторов/URL.
`FF_RDP_HOST` должен указывать на loopback; `FF_RDP_PORT` позволяет выбрать другой
локальный экземпляр. Во время теста не меняйте вкладки вручную.
Сам тест браузер не запускает.

Лицензия: [MIT](LICENSE). Backend: [ractive/ff-rdp](https://github.com/ractive/ff-rdp).
