# ✦ SE Agent

**版本 0.2.1-alpha（Alpha 測試版）** · Windows · macOS · Linux · Android · iOS

SE Agent（System Engineering Agent，指令 `seagent`）是自架的 AI 代理，以 Rust 撰寫。同一套程式碼跑在桌機、伺服器和手機上，是單一執行檔、依賴極少，可以長時間穩定地自動化工作。你用自然語言交代工作，它在**你自己的機器上**動手完成。模型只是它的「大腦」，可以隨時更換。

桌機版預設會開一台內建的虛擬機（Debian + Xfce + Chromium）當作**代理的電腦**。代理的指令、瀏覽器和滑鼠鍵盤操作都在那台 VM 裡進行，你在主控台看得到即時畫面，可以隨時接手。

> **Alpha 是什麼意思**：功能已經齊全，可以每天使用；但設定格式、指令名稱和網頁主控台在 0.x 期間還可能調整，升級前請先看 [CHANGELOG.md](CHANGELOG.md)。安裝檔還沒有程式碼簽章，Windows SmartScreen 和 macOS Gatekeeper 會跳警告。各平台驗證到什麼程度，見下方的[平台狀態](#平台狀態)。沒有備份的正式環境請勿使用。

## 目錄

- [下載與安裝](#下載與安裝)
- [第一次使用](#第一次使用)
- [平台狀態](#平台狀態)
- [它能做什麼](#它能做什麼)
- [代理的電腦：桌面 VM](#代理的電腦桌面-vm)
- [安全模型](#安全模型)
- [介面與整合](#介面與整合)
- [相容的生態與參考的上游](#相容的生態與參考的上游)
- [文件](#文件)
- [架構](#架構)
- [怎麼做出來的](#怎麼做出來的)
- [授權](#授權)

## 下載與安裝

所有安裝檔都在 [Releases](https://github.com/Noob-2006/System-Engineering-Agent/releases)，由 GitHub Actions 從打了標籤的原始碼建置。桌機和手機 App 的校驗碼在 `SHA256SUMS-app.txt`，Linux 命令列套件的校驗碼在 `SHA256SUMS`。

| 你用的是 | 下載 | 然後 |
|---|---|---|
| Windows 10（1809 以上）／11，x64 | `SE Agent_0.2.1-alpha_x64-setup.exe`（個人安裝，不需管理員）；或 `SE Agent_0.2.1-alpha_x64_<語言>.msi`（en-US、zh-TW、zh-CN） | 點兩下 →「其他資訊 → 仍要執行」（未簽章）→ 安裝完自動開設定精靈 |
| macOS 11 以上（Apple Silicon／Intel） | `SE Agent_0.2.1-alpha_universal.dmg` | 拖進「應用程式」→ 第一次在 App 上按右鍵「打開」（未公證；macOS 15 以後改到「系統設定 → 隱私權與安全性」按「強制打開」） |
| Linux 桌面（x86_64） | `.AppImage`（任何發行版）、`.deb`、`.rpm` | `chmod +x` 後執行，或 `sudo apt install ./SE-Agent_*.deb`／`sudo dnf install ./SE-Agent-*.rpm` |
| Linux 伺服器（x86_64／aarch64） | 一個指令（見下方） | 照印出的連結開設定精靈 |
| Android 8 以上（arm64） | `.apk` | 允許「安裝不明應用程式」後安裝 |
| iOS／iPadOS | 尚未上架（要 Apple 開發者憑證） | — |

App 第一次開啟時，會安裝並啟動背景服務，再在視窗裡開設定精靈。之後 App 常駐在系統匣，可以開啟主控台、暫停、重新啟動服務、設定登入時啟動、檢查更新。各平台的完整步驟、校驗方式、升級和疑難排解，見 [docs/install.md](docs/install.md)。

### Linux／伺服器：一個指令

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/Noob-2006/System-Engineering-Agent/releases/latest/download/install.sh | sh                 # 個人安裝，不需 sudo
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/Noob-2006/System-Engineering-Agent/releases/latest/download/install.sh | sudo sh -s -- --system  # 系統服務
```

這個指令會安裝 SE Agent、啟動背景服務和網頁主控台，最後印出一次性的設定精靈連結；有桌面環境時會自動開啟。

- 升級：重跑同一個指令。
- 解除安裝：`--uninstall` 會保留設定與資料，再加 `--purge` 一併清除。
- 指定版本：`--version v0.2.1-alpha`。
- 其他選項：`--port`、`--no-browser`、`--no-systemd`、`--local-binary PATH`、`--package FILE`、`--from-source`，`--help` 看全部。
- 另有 deb／rpm／pacman／apk 套件、npm、Nix flake、Dockerfile、Helm chart、Ansible role，以及離線安裝包（`seagent-offline-*.tar.gz`），見 [docs/install.md](docs/install.md) 和 [docs/enterprise/install.md](docs/enterprise/install.md)。

### 從原始碼

```bash
cargo build --release
packaging/install.sh --local-binary target/release/seagent
```

桌機 App（Tauri 2）的建置方式見 [apps/desktop/README.md](apps/desktop/README.md)，可選的編譯功能與大小預算見 [docs/BUILD.md](docs/BUILD.md)。

## 第一次使用

1. **設定精靈**（App 視窗、瀏覽器，或終端機的 `seagent setup`）：
   - 選模型供應商，貼上金鑰；金鑰只存進加密保管庫。
   - 從供應商的實際模型清單挑預設模型。
   - 設定管理者登入方式（本機權杖或 OIDC）、工作目錄、安全模式和聊天平台。

   完成後精靈自動鎖定，SE Agent 以新設定重啟，帶你進主控台。設定連結 30 分鐘內有效，只能用一次；要新的連結，執行 `seagent setup link`。
2. **代理的電腦**：在桌機上，SE Agent 一啟動就在背景準備桌面 VM，進度顯示在主控台頂列的「代理電腦」標籤。
   - 第一次要下載約 418 MB 的 Debian 映像並安裝桌面，約 5–20 分鐘，視網速而定。
   - 之後每次開機約 20 秒。
   - Windows 要先在「Windows 功能」勾選 Windows Hypervisor Platform 並重開機，才有硬體加速；App 的 VM 頁面有按鈕。
3. **開始交代工作**：在主控台的對話框輸入，或用命令列：

```bash
seagent run "整理這個資料夾的 README，列出待辦"   # 單次執行
seagent chat                                 # 互動對話
seagent tui                                  # 終端機介面
```

也可以跳過精靈：執行 `seagent secrets set ANTHROPIC_API_KEY`（或 OPENAI、DEEPSEEK 等），然後直接用預設模型。設定檔位置用 `seagent config path` 查：個人安裝在 `~/.config/seagent/config.toml`，系統服務在 `/etc/seagent/config.toml`。完整範例見 [docs/config-example.toml](docs/config-example.toml)。

## 平台狀態

0.2.1-alpha 在各平台的驗證程度：

- **實機**：在真的裝置上安裝並操作過。
- **建置**：CI 能產出安裝檔、內容也檢查過，但還沒在該平台的實體裝置上操作過。

| 平台 | 驗證程度 | 隔離用的 microVM | 桌面 VM（代理的電腦） | 備註 |
|---|---|---|---|---|
| Windows 10／11 x64 | **實機**（Windows 11 Home） | QEMU + WHPX（安裝包內附）；Hyper-V／VirtualBox 備援 | 安裝包內附 QEMU；自動建置、開機、VM 內代理服務與 Chromium 都驗證過 | 選裝虛擬顯示器驅動（Bot Desktop 30–60 fps）；未簽章 |
| Linux x86_64 | **實機**（WSL2 Ubuntu）；原生桌面發行版待測 | Firecracker（內附） | 用系統的 QEMU（deb／rpm 會建議安裝） | 主要開發平台 |
| macOS 11+ | **建置**（CI 在 macOS 機器上產出 universal `.dmg`，並檢查 App 內附 VM） | vfkit／Virtualization.framework | 用 Homebrew 或 MacPorts 的 QEMU；Apple Silicon 跑 x86_64 映像只能 TCG，很慢 | 未簽章、未公證 |
| Android 8+ arm64 | **建置** | AVF（支援的機型） | — | APK 目前是 debug 簽章；也能連到區域網路裡的電腦 |
| iOS／iPadOS | **建置**（CI 產出模擬器版） | 無（平台禁止），改用 WebAssembly 沙盒 | — | 完整代理在 App 行程內執行；實機版要 Apple 憑證 |

**iOS** 上，完整的代理以函式庫形式連進 App，在 App 自己的行程裡執行，不需要伺服器。iOS 只禁止兩件事：虛擬機和原生 shell 程式（不能開子程序、不能 JIT）。

- 指令改在 App 內的 WebAssembly 沙盒執行：wasmi 直譯器、內建 POSIX shell，以及釘選 SHA-256 的 WASI 版 coreutils、CPython、grep、sed、find、xargs。沙盒只看得到工作區和私有的 `/tmp`，沒有網路。
- App 在背景時，排程與心跳工作由 BGProcessingTask 在系統允許時執行。

詳見 [apps/desktop/MOBILE.md](apps/desktop/MOBILE.md)。

## 它能做什麼

- **系統與維運**：跑指令、改設定檔、看日誌、巡檢主機、透過 SSH 操作遠端伺服器；cron 排程與長期目標（goal）讓它定時回報或自動修復。
- **寫程式**：讀整個專案、修 bug、跑測試、開 git worktree 平行處理；也能在 Zed／JetBrains／Neovim 裡當編輯器代理（ACP）。
- **操作網頁與桌面**：瀏覽器工具（CDP）和電腦操作（computer use，靠截圖與模擬滑鼠鍵盤）。預設在桌面 VM 裡操作，Windows 和 macOS 的原生 App 也能透過系統的無障礙介面操作。
- **研究與內容**：網路搜尋、深度研究報告、文件抽取、圖片／影片／音樂生成、TTS／STT 語音、會議機器人，也能把多個工具串成流程。
- **聊天平台**：在 Telegram、LINE、Discord、Slack、Teams、飛書、Email 等 46 條通道上回應指令，當團隊或個人的值班助理。
- **多代理協作**：派子代理平行做事，有 kanban 看板和群聊房間；每個代理都有獨立的花費與資源上限。

| 面向 | 內容 |
|---|---|
| 代理核心 | 單次任務、互動對話、session 分支與搜尋、背景 daemon、cron、長期 goal、heartbeat、自動化建議、kanban、git worktree 平行工作區 |
| 子代理艦隊 | `delegate_task`／`sessions_spawn`／`agents_wait`：背景派遣、中途 steer 與取消、共用公告板、點對點訊息、檔案租約、每代理與全域的花費／磁碟／程序上限（`seagent fleet`）；群聊房間（`seagent rooms`）、Ralph 全新 context 輪次 |
| 執行環境 | 桌面 VM、microVM（Firecracker／QEMU／Hyper-V／Virtualization.framework）、gVisor、docker／podman、bwrap／Landlock、Windows AppContainer、macOS Seatbelt、ssh／modal／daytona；VM 快照與 undo；指令 broker 與出口 proxy |
| 模型 | OpenAI Chat／Responses、Anthropic、Gemini／Vertex、Bedrock、DeepSeek、Mistral、OpenRouter、OpenAI Agents API、外部代理 CLI（claude／gemini／codex、ACP）、本地 llama.cpp／Ollama／vLLM；訂閱登入、多金鑰輪替、fallback、輔助小模型、用量與帳單（`seagent usage`） |
| 工具 | 檔案、shell、背景 job、todo、skill、memory、web 搜尋與抓取、`deep_research`、瀏覽器與電腦操作、圖片／影片／音樂、TTS／STT、會議機器人、電話、排程、子代理 |
| 省 token | 工具漸進揭露；固定前綴快取（壓縮與 fork 子代理重播同一份前綴，舊輸出只在上下文吃緊時才裁剪）；自動壓縮；大型輸出溢寫到檔案。`seagent session stats --session <id>` 可以看每一步的快取命中率 |
| 稽核 | 每次工具呼叫的紀錄（主控台的 Logs 頁、`seagent logs tools\|export`）、稽核等級的匯出；主控台自己的輪詢會合併記錄，不會灌爆紀錄 |
| 法遵 | `privacy_notice`、`ai_label`、`[compliance] retention_days`、公部門 profile（`profile = "government"`）、`seagent data export\|delete`；台灣法規對照見 [docs/compliance-tw.md](docs/compliance-tw.md)，風險自評見 [docs/risk-assessment.md](docs/risk-assessment.md) |

## 代理的電腦：桌面 VM

桌面 VM（`[desktop_vm]`）是一台有畫面的 Debian 13 虛擬機，裡面有 Xfce 和 Chromium。它和隔離用的 microVM（`[exec.vm]`，無畫面、無網卡）是兩回事。

- **預設開啟**（`autostart = true`）：SE Agent 啟動時，背景服務會在第一次使用時建好映像，接著開機。VM 意外關掉會自動重開；你自己按停止，它就不會被拉回來。
- **代理在裡面工作**：
  - shell、程序和終端機工具，預設在執行中的桌面 VM 裡跑（`[exec] target = "auto"`），工作區會在主機和 VM 之間雙向同步。
  - 瀏覽器和電腦操作也預設在這台 VM 裡（`[computer_use] target = "vm:desk"`）。
  - VM 沒開或沒回應時，指令照舊在主機上執行，並提示一次。
- **看得到、接得手**：主控台的工作區以 30 fps（最高 60 fps）顯示 VM 畫面，可以直接接手操作。
- **映像下載**：同時從 cloud.debian.org 和多個官方鏡像站分段平行下載，可以續傳；斷網時會等網路恢復；最後一律比對固定的 sha256。
- **手動操作**：`seagent vm desktop build|start|stop|status|screenshot|input|reset`。

細節（設定鍵、主機需求、CDP 轉接、安全考量）見 [docs/desktop-vm.md](docs/desktop-vm.md)。

## 安全模型

- **只有兩道閘門**：提權（sudo、寫系統路徑），以及資料外送（秘密、個資離開機器）。其他事情直接做，不會一直跳出確認。
- **四級風險**：破壞性動作依四級風險處理。VM 裡的動作和可以回復的動作直接放行；只有「在 VM 外、而且不可回復」的動作一定會問人。
- **任務信封與具名授權**：一次性授權特定的路徑、主機、指令、花費和期限，到期自動失效。另有不可關閉的硬底線，不可信的內容會降權處理。
- **隔離等級一律明示**：隔離後端退回較弱的等級時，一定會說，不會靜默降級。Ubuntu 的 AppArmor 限制 user namespace 時，會偵測到並改用其他後端。
- **秘密**：存在加密保管庫裡，各平台用系統的金鑰存放處（DPAPI／Credential Manager、Keychain、Secret Service、TPM）。主控台用綁定裝置的憑證，會自動續期，不必重新登入。
- **緊急停止**：`seagent pause`。

完整說明見 [docs/MANUAL.md](docs/MANUAL.md) 和 [docs/SECURITY.md](docs/SECURITY.md)；回報漏洞請依 [SECURITY.md](SECURITY.md) 私下回報。

## 介面與整合

全部介面都講同一套協議 [ARP](docs/arp.md)：網頁主控台／PWA、桌機 App、終端機 TUI、編輯器（ACP）、裝置（node）、其他代理（A2A），以及 OpenAI 相容 API。

```bash
seagent daemon start                         # 背景服務：排程、goal、聊天通道、App Server
seagent service install                      # 登入或開機時自動啟動
seagent app open                             # 用瀏覽器 App 視窗開主控台
```

- **編輯器（ACP）**：在 Zed 的 `settings.json` 加上 `{ "agent_servers": { "seagent": { "command": "seagent", "args": ["acp"] } } }`。
- **裝置（node）**：在另一台機器執行 `seagent node --url ws://主機:8640/node`，再在主機上用 `seagent nodes approve <配對碼>` 核准。裝置能執行哪些指令，由裝置自己的 `node.toml` 決定。
- **區域網路配對**：Bonjour／NSD 自動探索，用 QR 碼或 8 碼配對，以 TLS 指紋釘選連線。手機可以連到家裡或公司的電腦。
- **OpenAI 相容 API**：`POST http://127.0.0.1:8640/v1/chat/completions`，帶 `Authorization: Bearer <token>`。
- **聊天平台**：在設定檔加入 `[channels.<名稱>]`，設定 `kind = "telegram"` 等。46 條通道中，43 個平台由 `seagent-channel` 內建，webchat、A2A、OpenAI 相容 API 由 App Server 提供。對陌生人預設拒絕；設成 `dm_policy = "pairing"` 時，對方會拿到配對碼，由你用 `seagent pairing approve` 核准。逐平台狀態見 [docs/parity.md](docs/parity.md)，設定方式見 [docs/channel-guide.md](docs/channel-guide.md)。
- **MCP**：client 和 server 都支援。
  - 傳輸：stdio、Streamable HTTP、舊版 SSE。
  - 認證：OAuth 2.1（PKCE、動態註冊、device flow）。
  - 其他：sampling、elicitation、roots、成本上限。

```bash
seagent mcp add files -- npx -y @modelcontextprotocol/server-filesystem ~/docs
seagent mcp login github                     # OAuth；沒有瀏覽器的主機加 --device
seagent mcp serve                            # 讓其他代理使用 seagent 的工具
```

## 相容的生態與參考的上游

SE Agent 是獨立實作，不是任何專案的分支。它讀得懂下列生態的格式，搬過來不必重寫。匯入時預設**不**搬金鑰；要搬，請加 `--migrate-secrets`（金鑰會寫進加密保管庫）。

| 生態 | 相容到什麼程度 | 文件 |
|---|---|---|
| Claude Code | 技能目錄（`.claude/skills`、`SKILL.md`）、hooks（同樣的事件與 JSON）、settings、外掛市集格式；也能把 `claude` CLI 當成外部代理呼叫 | [other-ecosystems.md](docs/ecosystem/other-ecosystems.md) |
| OpenClaw | 技能 metadata、設定、外掛、ClawHub、node 裝置；可用 `seagent migrate from openclaw` 匯入 | [openclaw-compat.md](docs/ecosystem/openclaw-compat.md) |
| Hermes Agent | 技能、設定、`plugin.yaml` 外掛、optional-mcps；可用 `seagent migrate from hermes` 匯入 | [hermes-compat.md](docs/ecosystem/hermes-compat.md) |
| LangChain／LangGraph | 經 OpenAI 相容 API、MCP 或 A2A 接入；`graph-flow` 工具提供 LangGraph 式的持久化流程；LangSmith 可以經 OTLP 收追蹤 | [langchain-integration.md](docs/ecosystem/langchain-integration.md) |

功能設計逐項對照了下列開源代理專案，並把值得的部分以 Rust 重新實作。這些專案都是 MIT 授權，出處與版權聲明見 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

| 上游 | 對照版本 | 借鏡了什麼 |
|---|---|---|
| [OpenClaw](https://github.com/openclaw/openclaw) | `8a186ad` | 聊天通道與閘道、node 裝置、技能與外掛格式、瀏覽器沙盒、TTS |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent) | `90d6b98` | 聊天平台轉接器、終端機介面與 slash 指令、檢查點與 `/undo`、語音模式、秘密遮蔽 |
| DeepSeek Harness（dsh） | `fcc0ad4` | 前綴快取策略、逐步 TTFT 與快取統計、DeepSeek Messages 路由 |
| LangChain v1／LangGraph／deepagents／LangSmith | — | 代理迴圈、checkpoint 與 interrupt、持久化流程、追蹤格式 |

逐項落差報告在 [docs/parity-agent/](docs/parity-agent/)。上游會把工作階段紀錄上傳到模型供應商的功能（例如 dsh 的 `dsh_session_log`），**刻意沒有**移植。

## 文件

| 文件 | 內容 |
|---|---|
| [docs/install.md](docs/install.md) | 各平台安裝、校驗、升級、解除安裝、疑難排解 |
| [docs/MANUAL.md](docs/MANUAL.md) | 使用者手冊：通道、安全模型、VM、艦隊、技能與外掛、模型與成本 |
| [docs/desktop-vm.md](docs/desktop-vm.md) | 桌面 VM（代理的電腦） |
| [docs/faq.md](docs/faq.md) | 常見問題 |
| [docs/cross-platform.md](docs/cross-platform.md) | 跨平台設計：每個 Linux 功能在 Windows／macOS／手機上的替代做法 |
| [docs/paths.md](docs/paths.md) | 設定、資料、快取放在哪裡 |
| [docs/enterprise/](docs/enterprise/) | 企業部署、離線安裝、套件庫 |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、[docs/BUILD.md](docs/BUILD.md) | 分層規則、建置選項 |
| [CHANGELOG.md](CHANGELOG.md) | 每個版本改了什麼 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 參與開發（每個 commit 需 `Signed-off-by`）；行為準則見 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) |

## 架構

| crate | 職責 |
|---|---|
| `seagent-core` | 型別、工具與 provider 介面 |
| `seagent-kernel` | session 迴圈、閘門、事件紀錄、daemon、ARP |
| `seagent-model` | 各家模型的 API 格式 |
| `seagent-tools` | 檔案、shell、job、todo、skill、memory、web、子代理 |
| `seagent-plugin` | MCP client／server、OAuth、外掛協議 |
| `seagent-net` | TLS、WebSocket、HTTP 伺服器、加密工具 |
| `seagent-channel` | 43 個聊天平台、存取政策、配對、投遞 |
| `seagent-app` | App Server、Web UI、TUI、ACP、node、A2A、OpenAI API |
| `seagent-vm` | microVM／容器／namespace 後端、桌面 VM、隔離等級、映像與快照 |
| `seagent-media` | 視覺、文件抽取、圖片／影片／音樂生成、TTS／STT、會議 |
| `seagent-browser` | CDP 瀏覽器控制與擷取 |
| `seagent` | CLI |
| `apps/desktop` | Tauri 2 的桌機與手機 App 外殼 |

分層規則、功能如何自我登記，以及單檔與單一目錄的大小上限，寫在 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，並由 `cargo test -p seagent --test arch_lint` 強制執行。

## 怎麼做出來的

SE Agent 主要由 AI 程式代理寫成，過程由人類指揮與審查：

- **DeepSeek Harness**：前期的核心、工具與上游對照。
- **Claude Code**（Anthropic）：跨平台移植（Windows／macOS／Android／iOS）、安裝檔與內建 VM、文件，以及 Windows 實機驗證。

每個 commit 都經過測試，由代理寫成的 commit 帶有 `Co-Authored-By` 標記。

## 授權

Apache License 2.0（[LICENSE](LICENSE)、[NOTICE](NOTICE)），0.2.0-alpha 以前的版本以 MIT 發布。部分程式轉寫自 OpenClaw、Hermes Agent 和 DeepSeek Harness；這些來源是 MIT 授權，版權與授權聲明依 MIT 的要求保留，見 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。Windows 安裝包內附的 QEMU 是 GPL-2.0，原始碼位置寫在安裝包的 `vm\bundle.json` 和 [packaging/vm/README.md](packaging/vm/README.md)。
