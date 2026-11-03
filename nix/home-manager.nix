self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.tabbify;
  toml = pkgs.formats.toml { };
  tabbify = lib.getExe cfg.package;
  # The markers `tabbify install` uses, so `tabbify doctor` sees the integration.
  block = part: line: ''
    # >>> tabbify ${part} >>>
    ${line}
    # <<< tabbify ${part} <<<
  '';
  posix = shell: part: block part ''eval "$(${tabbify} init ${shell} ${part})"'';
  fish = part: block part "eval (${tabbify} init fish ${part} | string collect)";
in
{
  options.programs.tabbify = {
    enable = lib.mkEnableOption "tabbify, IDE-style autocomplete for the terminal";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The tabbify package to use.";
    };

    settings = lib.mkOption {
      inherit (toml) type;
      default = { };
      description = "Written to {file}`$XDG_CONFIG_HOME/tabbify/config.toml`. `tabbify config` lists every option.";
    };

    enableBashIntegration = lib.hm.shell.mkBashIntegrationOption { inherit config; };
    enableFishIntegration = lib.hm.shell.mkFishIntegrationOption { inherit config; };
    enableZshIntegration = lib.hm.shell.mkZshIntegrationOption { inherit config; };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."tabbify/config.toml" = lib.mkIf (cfg.settings != { }) {
      source = toml.generate "tabbify-config.toml" cfg.settings;
    };

    # `pre` re-runs the shell inside tabbify, so it goes first; `post` goes last.
    programs.zsh.initContent = lib.mkIf cfg.enableZshIntegration (
      lib.mkMerge [
        (lib.mkBefore (posix "zsh" "pre"))
        (lib.mkAfter (posix "zsh" "post"))
      ]
    );
    programs.bash.bashrcExtra = lib.mkIf cfg.enableBashIntegration (lib.mkBefore (posix "bash" "pre"));
    programs.bash.initExtra = lib.mkIf cfg.enableBashIntegration (lib.mkAfter (posix "bash" "post"));
    programs.fish.shellInit = lib.mkIf cfg.enableFishIntegration (lib.mkBefore (fish "pre"));
    programs.fish.shellInitLast = lib.mkIf cfg.enableFishIntegration (lib.mkAfter (fish "post"));
  };
}
