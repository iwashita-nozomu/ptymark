# Shell and rich-plugin compatibility

## Scope

`ptymark` must coexist with shell frameworks, prompts, line editors, completion
menus, history search, directory hooks, and full-screen fuzzy tools without
taking ownership of their configuration or terminal controls.

The compatibility contract is deliberately narrower than claiming that every
third-party release is vendored and executed in CI:

- the installer must not edit Bash, Zsh, Fish, PowerShell, or Nushell profile files;
- `ptymark -- COMMAND` must preserve shell-related environment variables;
- prompt, right-prompt, line-editor, completion, OSC, DCS, carriage-return, and
  alternate-screen bytes must remain byte-exact;
- full-screen and cursor-addressed interfaces must bypass semantic rendering;
- a complete explicit Mermaid or block-math fence on a later safe line may still
  render normally;
- third-party key bindings, hooks, and plugin-manager state remain owned by the shell.

The interactive PTY host is not implemented yet. Current live checks cover the
launcher/installer boundary and the reusable pre-display pipeline. When the PTY
host is added, the same compatibility suite is a mandatory merge gate.

## Verification levels

`contract-verified` means the integration is mapped to a terminal-behavior
profile that is exercised by `tests/shell_plugin_compatibility.rs`. It does not
mean that ptymark pins or redistributes that upstream project.

The live CI additionally checks:

- Linux/macOS/Windows native Rust tests;
- PowerShell, cmd.exe, and Git Bash installer frontends;
- unchanged shell profile files on Unix and Windows;
- shell-hook environment propagation through `ptymark -- COMMAND`;
- PSReadLine availability on the Windows runner;
- packaged Linux, macOS, and Windows executables;
- WezTerm launcher configuration append behavior.

## Behavior profiles

| Profile | Representative behavior | ptymark treatment |
| --- | --- | --- |
| `safe-text` | glyph-rich listings and normal command output | pass through; only explicit standalone fences are candidates |
| `hook-only` | environment, directory, and plugin-manager hooks | no shell files or hook state are modified |
| `prompt-control` | SGR, OSC title/cwd/shell-integration markers | byte-exact raw path for the control-bearing line |
| `right-prompt` | cursor save/restore and right-column placement | byte-exact raw path |
| `line-editor` | syntax color, autosuggestions, bracketed paste, redraw | byte-exact raw path |
| `completion-menu` | erase-line, cursor movement, multi-line menus | byte-exact raw path |
| `alternate-screen` | fzf, history UI, file browser, mouse reporting | complete bypass until alternate-screen exit |
| `progress-line` | carriage-return and backspace updates | byte-exact raw path |

## Reviewed integration matrix

The inventories below are also stored as machine-readable TSV files under
`compat/shell-integrations/`. CI requires exactly twenty reviewed entries for
each shell, unique names, supported behavior profiles, and HTTPS upstream
references.

### Bash

| Integration | Category | Profile | Verification | Upstream |
| --- | --- | --- | --- | --- |
| Bash-it | framework | `prompt-control` | contract-verified | [source](https://github.com/Bash-it/bash-it) |
| Oh My Bash | framework | `prompt-control` | contract-verified | [source](https://github.com/ohmybash/oh-my-bash) |
| ble.sh | line-editor | `line-editor` | contract-verified | [source](https://github.com/akinomyoga/ble.sh) |
| Liquidprompt | prompt | `prompt-control` | contract-verified | [source](https://github.com/liquidprompt/liquidprompt) |
| bash-git-prompt | prompt | `prompt-control` | contract-verified | [source](https://github.com/magicmonty/bash-git-prompt) |
| powerline-shell | prompt | `prompt-control` | contract-verified | [source](https://github.com/b-ryan/powerline-shell) |
| Starship | prompt | `prompt-control` | contract-verified | [source](https://github.com/starship/starship) |
| Oh My Posh | prompt | `prompt-control` | contract-verified | [source](https://github.com/JanDeDobbeleer/oh-my-posh) |
| bash-preexec | hook | `hook-only` | contract-verified | [source](https://github.com/rcaloras/bash-preexec) |
| bash-completion | completion | `completion-menu` | contract-verified | [source](https://github.com/scop/bash-completion) |
| fzf | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/junegunn/fzf) |
| fzf-git.sh | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/junegunn/fzf-git.sh) |
| Atuin | history | `alternate-screen` | contract-verified | [source](https://github.com/atuinsh/atuin) |
| zoxide | navigation | `hook-only` | contract-verified | [source](https://github.com/ajeetdsouza/zoxide) |
| direnv | environment | `hook-only` | contract-verified | [source](https://github.com/direnv/direnv) |
| autojump | navigation | `hook-only` | contract-verified | [source](https://github.com/wting/autojump) |
| Carapace | completion | `completion-menu` | contract-verified | [source](https://github.com/carapace-sh/carapace-bin) |
| Navi | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/denisidoro/navi) |
| Broot | file-navigation | `alternate-screen` | contract-verified | [source](https://github.com/Canop/broot) |
| Mise | environment | `hook-only` | contract-verified | [source](https://github.com/jdx/mise) |

### Zsh

| Integration | Category | Profile | Verification | Upstream |
| --- | --- | --- | --- | --- |
| Oh My Zsh | framework | `prompt-control` | contract-verified | [source](https://github.com/ohmyzsh/ohmyzsh) |
| Prezto | framework | `prompt-control` | contract-verified | [source](https://github.com/sorin-ionescu/prezto) |
| Zim | framework | `prompt-control` | contract-verified | [source](https://github.com/zimfw/zimfw) |
| Zinit | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/zdharma-continuum/zinit) |
| Antidote | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/mattmc3/antidote) |
| zplug | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/zplug/zplug) |
| Znap | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/marlonrichert/zsh-snap) |
| Sheldon | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/rossmacarthur/sheldon) |
| zsh-autosuggestions | line-editor | `line-editor` | contract-verified | [source](https://github.com/zsh-users/zsh-autosuggestions) |
| zsh-syntax-highlighting | line-editor | `line-editor` | contract-verified | [source](https://github.com/zsh-users/zsh-syntax-highlighting) |
| Fast Syntax Highlighting | line-editor | `line-editor` | contract-verified | [source](https://github.com/zdharma-continuum/fast-syntax-highlighting) |
| zsh-autocomplete | completion | `completion-menu` | contract-verified | [source](https://github.com/marlonrichert/zsh-autocomplete) |
| fzf-tab | completion | `alternate-screen` | contract-verified | [source](https://github.com/Aloxaf/fzf-tab) |
| Powerlevel10k | prompt | `right-prompt` | contract-verified | [source](https://github.com/romkatv/powerlevel10k) |
| Pure | prompt | `right-prompt` | contract-verified | [source](https://github.com/sindresorhus/pure) |
| Spaceship | prompt | `prompt-control` | contract-verified | [source](https://github.com/spaceship-prompt/spaceship-prompt) |
| Starship | prompt | `prompt-control` | contract-verified | [source](https://github.com/starship/starship) |
| Oh My Posh | prompt | `prompt-control` | contract-verified | [source](https://github.com/JanDeDobbeleer/oh-my-posh) |
| Atuin | history | `alternate-screen` | contract-verified | [source](https://github.com/atuinsh/atuin) |
| zoxide | navigation | `hook-only` | contract-verified | [source](https://github.com/ajeetdsouza/zoxide) |

### Fish

| Integration | Category | Profile | Verification | Upstream |
| --- | --- | --- | --- | --- |
| Fisher | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/jorgebucaran/fisher) |
| Oh My Fish | framework | `prompt-control` | contract-verified | [source](https://github.com/oh-my-fish/oh-my-fish) |
| Fundle | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/tuvistavie/fundle) |
| Tide | prompt | `right-prompt` | contract-verified | [source](https://github.com/IlanCosman/tide) |
| Hydro | prompt | `prompt-control` | contract-verified | [source](https://github.com/jorgebucaran/hydro) |
| Pure | prompt | `right-prompt` | contract-verified | [source](https://github.com/pure-fish/pure) |
| Starship | prompt | `prompt-control` | contract-verified | [source](https://github.com/starship/starship) |
| Oh My Posh | prompt | `prompt-control` | contract-verified | [source](https://github.com/JanDeDobbeleer/oh-my-posh) |
| fzf.fish | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/PatrickF1/fzf.fish) |
| autopair.fish | line-editor | `line-editor` | contract-verified | [source](https://github.com/jorgebucaran/autopair.fish) |
| replay.fish | environment | `hook-only` | contract-verified | [source](https://github.com/jorgebucaran/replay.fish) |
| nvm.fish | environment | `hook-only` | contract-verified | [source](https://github.com/jorgebucaran/nvm.fish) |
| done | notification | `progress-line` | contract-verified | [source](https://github.com/franciscolourenco/done) |
| sponge | history | `hook-only` | contract-verified | [source](https://github.com/meaningful-ooo/sponge) |
| pisces | line-editor | `line-editor` | contract-verified | [source](https://github.com/laughedelic/pisces) |
| bass | environment | `hook-only` | contract-verified | [source](https://github.com/edc/bass) |
| Atuin | history | `alternate-screen` | contract-verified | [source](https://github.com/atuinsh/atuin) |
| zoxide | navigation | `hook-only` | contract-verified | [source](https://github.com/ajeetdsouza/zoxide) |
| direnv | environment | `hook-only` | contract-verified | [source](https://github.com/direnv/direnv) |
| Carapace | completion | `completion-menu` | contract-verified | [source](https://github.com/carapace-sh/carapace-bin) |

### PowerShell

| Integration | Category | Profile | Verification | Upstream |
| --- | --- | --- | --- | --- |
| PSReadLine | line-editor | `line-editor` | contract-verified | [source](https://github.com/PowerShell/PSReadLine) |
| posh-git | prompt | `prompt-control` | contract-verified | [source](https://github.com/dahlbyk/posh-git) |
| Oh My Posh | prompt | `right-prompt` | contract-verified | [source](https://github.com/JanDeDobbeleer/oh-my-posh) |
| Starship | prompt | `prompt-control` | contract-verified | [source](https://github.com/starship/starship) |
| Terminal-Icons | display | `safe-text` | contract-verified | [source](https://github.com/devblackops/Terminal-Icons) |
| PSFzf | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/kelleyma49/PSFzf) |
| CompletionPredictor | prediction | `line-editor` | contract-verified | [source](https://github.com/PowerShell/CompletionPredictor) |
| Az.Tools.Predictor | prediction | `line-editor` | contract-verified | [source](https://github.com/Azure/azure-powershell) |
| WinGet CommandNotFound | command-discovery | `safe-text` | contract-verified | [source](https://github.com/microsoft/winget-command-not-found) |
| PSCompletions | completion | `completion-menu` | contract-verified | [source](https://github.com/abgox/PSCompletions) |
| Inshellisense | completion | `alternate-screen` | contract-verified | [source](https://github.com/microsoft/inshellisense) |
| Atuin | history | `alternate-screen` | contract-verified | [source](https://github.com/atuinsh/atuin) |
| zoxide | navigation | `hook-only` | contract-verified | [source](https://github.com/ajeetdsouza/zoxide) |
| direnv | environment | `hook-only` | contract-verified | [source](https://github.com/direnv/direnv) |
| Carapace | completion | `completion-menu` | contract-verified | [source](https://github.com/carapace-sh/carapace-bin) |
| fzf | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/junegunn/fzf) |
| Navi | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/denisidoro/navi) |
| Broot | file-navigation | `alternate-screen` | contract-verified | [source](https://github.com/Canop/broot) |
| Mise | environment | `hook-only` | contract-verified | [source](https://github.com/jdx/mise) |
| Pscx | shell-extension | `safe-text` | contract-verified | [source](https://github.com/Pscx/Pscx) |

### Nushell

| Integration | Category | Profile | Verification | Upstream |
| --- | --- | --- | --- | --- |
| nu_scripts | module-pack | `safe-text` | contract-verified | [source](https://github.com/nushell/nu_scripts) |
| Nupm | plugin-manager | `hook-only` | contract-verified | [source](https://github.com/nushell/nupm) |
| Starship | prompt | `prompt-control` | contract-verified | [source](https://github.com/starship/starship) |
| Oh My Posh | prompt | `right-prompt` | contract-verified | [source](https://github.com/JanDeDobbeleer/oh-my-posh) |
| Atuin | history | `alternate-screen` | contract-verified | [source](https://github.com/atuinsh/atuin) |
| zoxide | navigation | `hook-only` | contract-verified | [source](https://github.com/ajeetdsouza/zoxide) |
| Carapace | completion | `completion-menu` | contract-verified | [source](https://github.com/carapace-sh/carapace-bin) |
| direnv | environment | `hook-only` | contract-verified | [source](https://github.com/direnv/direnv) |
| Mise | environment | `hook-only` | contract-verified | [source](https://github.com/jdx/mise) |
| fzf | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/junegunn/fzf) |
| Navi | fuzzy-ui | `alternate-screen` | contract-verified | [source](https://github.com/denisidoro/navi) |
| Broot | file-navigation | `alternate-screen` | contract-verified | [source](https://github.com/Canop/broot) |
| nu_plugin_gstat | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_gstat) |
| nu_plugin_query | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_query) |
| nu_plugin_polars | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_polars) |
| nu_plugin_formats | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_formats) |
| nu_plugin_inc | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_inc) |
| nu_plugin_custom_values | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_custom_values) |
| nu_plugin_example | plugin | `safe-text` | contract-verified | [source](https://github.com/nushell/nushell/tree/main/crates/nu_plugin_example) |
| Pay Respects | command-correction | `safe-text` | contract-verified | [source](https://github.com/iffse/pay-respects) |

## Extension rule

A new integration does not require a special-case code path merely because it
has a new brand name. Add a new behavior profile only when the tool emits a
terminal interaction not represented above. Such a profile must include:

1. a byte fixture with arbitrary chunk-boundary coverage;
2. expected safe/raw classification;
3. a pipeline test proving that no partial display bytes are emitted;
4. a documented fallback when the terminal behavior is unknown;
5. a live host test once the interactive PTY host exists.
