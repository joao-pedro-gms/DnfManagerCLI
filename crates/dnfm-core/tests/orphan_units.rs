//! Testes da heurística de órfãos com capabilities reais do daemon.
//!
//! As strings abaixo foram capturadas de `Rpm.list` no `dnf5daemon-server`
//! desta máquina (8.025 instalados, 312.207 provides, 126.395 requires), não
//! são exemplos inventados: `mesa-vulkan-drivers` e `webkitgtk6.0` apareciam
//! como órfãos justamente por causa do formato. Os bloqueios de dependência
//! de arquivo vêm do `rpm -e --test`: rpm recusa remover os sete porque
//! outros pacotes exigem caminhos que eles possuem.

use dnfm_core::capability::{
    capability_name, file_owners, is_orphan_reason, orphan_keys, required_keys, requirement_names,
    InstalledRecord,
};
use dnfm_core::model::Reason;

fn registro(key: &str, reason: Reason, provides: &[&str], requires: &[&str]) -> InstalledRecord {
    InstalledRecord {
        key: key.to_string(),
        reason,
        provides: provides.iter().map(|s| s.to_string()).collect(),
        requires: requires.iter().map(|s| s.to_string()).collect(),
        files: Vec::new(),
    }
}

fn registro_com_arquivos(
    key: &str,
    reason: Reason,
    provides: &[&str],
    requires: &[&str],
    files: &[&str],
) -> InstalledRecord {
    let mut record = registro(key, reason, provides, requires);
    record.files = files.iter().map(|s| s.to_string()).collect();
    record
}

#[test]
fn nome_da_capability_descarta_versao_arquitetura_e_rpmlib() {
    assert_eq!(
        capability_name("libdrm(x86-64) = 2.4.124-1.fc45").as_deref(),
        Some("libdrm")
    );
    assert_eq!(
        capability_name("mesa-vulkan-drivers(x86-32) = 26.2.6-1.fc45").as_deref(),
        Some("mesa-vulkan-drivers")
    );
    assert_eq!(
        capability_name("webkitgtk6.0 >= 2.40.1").as_deref(),
        Some("webkitgtk6.0")
    );
    assert_eq!(
        capability_name("libc.so.6(GLIBC_2.14)(64bit)").as_deref(),
        Some("libc.so.6")
    );
    assert_eq!(
        capability_name("javascriptcoregtk6.0(x86-64) = 2.54.2-1.fc45").as_deref(),
        Some("javascriptcoregtk6.0")
    );
    assert_eq!(capability_name("rpmlib(FileDigests) <= 4.6.0-1"), None);
    assert_eq!(capability_name("rpmlib(PayloadIsZstd) <= 5.4.18-1"), None);
    assert_eq!(capability_name("   "), None);
}

#[test]
fn rich_dep_vira_alternativas() {
    assert_eq!(
        requirement_names("(libnotify or libnotify4)"),
        vec!["libnotify".to_string(), "libnotify4".to_string()]
    );
    assert_eq!(
        requirement_names("(alsa-plugins-pulseaudio if pulseaudio)"),
        vec![
            "alsa-plugins-pulseaudio".to_string(),
            "pulseaudio".to_string()
        ]
    );
    let kde = requirement_names(
        "(kde-cli-tools or kde-cli-tools5 or kde-runtime or trash-cli or glib2 or gvfs-client)",
    );
    assert_eq!(kde.len(), 6);
    assert!(kde.contains(&"trash-cli".to_string()));
    // O `if` do linux-firmware repete o próprio nome: uma alternativa só.
    assert_eq!(
        requirement_names("(linux-firmware = 20261019-1.fc45 if linux-firmware)"),
        vec!["linux-firmware".to_string()]
    );
    // Parêntese que não envolve a expressão toda não é rich dep.
    assert_eq!(
        requirement_names("rpmlib(BuiltinLuaScripts) <= 4.2.2-1"),
        Vec::<String>::new()
    );
}

#[test]
fn require_com_versao_casa_com_provide_de_outra_versao() {
    // foliate exige `webkitgtk6.0 >= 2.40.1`; o provide é `webkitgtk6.0 = 2.54.2`,
    // que na comparação literal nunca casava.
    let registros = vec![
        registro(
            "webkitgtk6.0.x86_64",
            Reason::Dependency,
            &[
                "bundled(angle)",
                "libwebkitgtk-6.0.so.4()(64bit)",
                "webkitgtk6.0 = 2.54.2-1.fc45",
                "webkitgtk6.0(x86-64) = 2.54.2-1.fc45",
            ],
            &[
                "bubblewrap",
                "libc.so.6()(64bit)",
                "rpmlib(FileDigests) <= 4.6.0-1",
            ],
        ),
        registro(
            "foliate.noarch",
            Reason::User,
            &["application()", "foliate = 3.3.0-4.fc45"],
            &["gjs >= 1.76", "webkitgtk6.0 >= 2.40.1"],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.contains("webkitgtk6.0.x86_64"));
    assert!(orphan_keys(&registros).is_empty());
}

#[test]
fn provide_de_arquitetura_casa_com_require_de_outra() {
    // steam exige `mesa-vulkan-drivers` e `mesa-vulkan-drivers(x86-32)`; os
    // provides são `mesa-vulkan-drivers(x86-64)` e `mesa-vulkan-drivers(x86-32)`.
    let registros = vec![
        registro(
            "mesa-vulkan-drivers.x86_64",
            Reason::Group,
            &[
                "libvulkan_lvp.so()(64bit)",
                "mesa-vulkan-drivers = 26.2.6-1.fc45",
                "mesa-vulkan-drivers(x86-64) = 26.2.6-1.fc45",
            ],
            &["libc.so.6()(64bit)", "libLLVM.so.23.1(LLVM_23.1)(64bit)"],
        ),
        registro(
            "mesa-vulkan-drivers.i686",
            Reason::Group,
            &[
                "libvulkan_lvp.so",
                "mesa-vulkan-drivers = 26.2.6-1.fc45",
                "mesa-vulkan-drivers(x86-32) = 26.2.6-1.fc45",
            ],
            &["libc.so.6", "libLLVM.so.23.1"],
        ),
        registro(
            "steam.x86_64",
            Reason::Dependency,
            &["application(steam.desktop)", "steam = 1.0.0.87-5.fc45"],
            &[
                "(udev-joystick-blacklist-rm or udev-joystick-blacklist)",
                "mesa-vulkan-drivers",
                "mesa-vulkan-drivers(x86-32)",
            ],
        ),
        registro(
            "lutris.x86_64",
            Reason::User,
            &["lutris = 0.5.22-5.fc45"],
            &["mesa-vulkan-drivers", "/usr/bin/python3"],
        ),
        registro(
            "steam-arch-transition.noarch",
            Reason::User,
            &["steam-arch-transition = 1.0.0.87-5.fc45"],
            &["steam = 1.0.0.87-5.fc45"],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.contains("mesa-vulkan-drivers.x86_64"));
    assert!(requeridos.contains("mesa-vulkan-drivers.i686"));
    assert!(orphan_keys(&registros).is_empty());
}

#[test]
fn rich_dep_mantem_o_provedor_da_alternativa() {
    // Goose pede `(libnotify or libnotify4)`; sem expandir a rich dep a entrada
    // virava string vazia e libnotify caía como órfão, mesmo sendo dependência.
    let registros = vec![
        registro(
            "libnotify.x86_64",
            Reason::Dependency,
            &[
                "libnotify = 0.8.9-1.fc45",
                "libnotify(x86-64) = 0.8.9-1.fc45",
                "libnotify.so.4()(64bit)",
            ],
            &["libc.so.6()(64bit)"],
        ),
        registro(
            "Goose.noarch",
            Reason::User,
            &["Goose = 3.16.2-1.fc45"],
            &[
                "(libnotify or libnotify4)",
                "(kde-cli-tools or kde-cli-tools5 or kde-runtime or trash-cli or glib2 or gvfs-client)",
            ],
        ),
    ];

    assert!(required_keys(&registros).contains("libnotify.x86_64"));
    assert!(orphan_keys(&registros).is_empty());
}

#[test]
fn rpmlib_nao_segura_pacote_como_requerido() {
    // Todo instalado exige `rpmlib(...)`; isso não pode contar como dependência.
    let registros = vec![
        registro(
            "tini-static.x86_64",
            Reason::Dependency,
            &[
                "tini-static = 0.19.0-12.fc44",
                "tini-static(x86-64) = 0.19.0-12.fc44",
            ],
            &["rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro(
            "outro.noarch",
            Reason::User,
            &["outro = 1-1.fc45"],
            &[
                "rpmlib(FileDigests) <= 4.6.0-1",
                "rpmlib(PayloadIsZstd) <= 5.4.18-1",
            ],
        ),
    ];

    assert_eq!(orphan_keys(&registros), vec!["tini-static.x86_64"]);
}

#[test]
fn motivo_de_grupo_ou_de_usuario_nao_vira_orfao() {
    // Nenhum instalado requer estes dois (rpm -q --whatrequires volta vazio):
    // o que os tira da lista é o motivo Group, o mesmo que o `dnf5 autoremove`
    // respeita.
    let registros = vec![
        registro(
            "plasma-workspace-wallpapers.noarch",
            Reason::Group,
            &["plasma-workspace-wallpapers = 6.8.3-1.fc45"],
            &["kde-filesystem", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro(
            "nvidia-gpu-firmware.noarch",
            Reason::Group,
            &["nvidia-gpu-firmware = 20261019-1.fc45"],
            &["linux-firmware-whence = 20261019-1.fc45"],
        ),
        registro(
            "vlc.x86_64",
            Reason::User,
            &["vlc = 3.0.21-20.fc45"],
            &["libc.so.6()(64bit)"],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.is_empty(), "ninguém requer estes pacotes");
    assert!(orphan_keys(&registros).is_empty());

    assert!(!is_orphan_reason(Reason::Group));
    assert!(!is_orphan_reason(Reason::User));
    assert!(!is_orphan_reason(Reason::Unknown));
    assert!(is_orphan_reason(Reason::Dependency));
    assert!(is_orphan_reason(Reason::WeakDependency));
}

#[test]
fn dependencia_verdadeiramente_solta_continua_orfao() {
    let registros = vec![
        registro(
            "SDL2_net.x86_64",
            Reason::Dependency,
            &["SDL2_net = 2.0.2-7.fc45", "libSDL2_net-2.0.so.0()(64bit)"],
            &["libc.so.6()(64bit)", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro(
            "dbus-x11.x86_64",
            Reason::Dependency,
            &[
                "dbus-x11 = 1:1.16.4-2.fc45",
                "dbus-x11(x86-64) = 1:1.16.4-2.fc45",
            ],
            &["dbus-daemon", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro(
            "dbus-daemon.x86_64",
            Reason::Dependency,
            &["dbus-daemon = 1:1.16.4-2.fc45"],
            &["libc.so.6()(64bit)"],
        ),
    ];

    // dbus-daemon é requerido; SDL2_net não.
    assert_eq!(
        orphan_keys(&registros),
        vec!["SDL2_net.x86_64".to_string(), "dbus-x11.x86_64".to_string()]
    );
}

#[test]
fn chaves_saem_sem_repeticao_e_em_ordem() {
    let registros = vec![
        registro(
            "b.x86_64",
            Reason::Dependency,
            &["b = 1-1.fc45"],
            &["libc.so.6()(64bit)"],
        ),
        registro(
            "a.noarch",
            Reason::WeakDependency,
            &["a = 1-1.fc45"],
            &["libc.so.6()(64bit)"],
        ),
        registro(
            "b.x86_64",
            Reason::Dependency,
            &["b = 1-1.fc45"],
            &["libc.so.6()(64bit)"],
        ),
    ];

    assert_eq!(
        orphan_keys(&registros),
        vec!["a.noarch".to_string(), "b.x86_64".to_string()]
    );
}

#[test]
fn require_de_arquivo_segura_o_dono_systemd_sysusers() {
    // `rpm -e --test systemd-sysusers` falha: rpm-libs exige
    // `/usr/bin/systemd-sysusers`, que é arquivo do pacote e não capability.
    let registros = vec![
        registro(
            "zz-controle.x86_64",
            Reason::Dependency,
            &["zz-controle = 1-1.fc45"],
            &["libc.so.6()(64bit)", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro_com_arquivos(
            "systemd-sysusers.x86_64",
            Reason::Dependency,
            &[
                "systemd-sysusers = 262.1-1.fc45",
                "systemd-sysusers(x86-64) = 262.1-1.fc45",
            ],
            &[
                "rpmlib(FileDigests) <= 4.6.0-1",
                "rpmlib(PayloadIsZstd) <= 5.4.18-1",
            ],
            &["/usr/bin/systemd-sysusers"],
        ),
        registro(
            "rpm-libs.x86_64",
            Reason::User,
            &["rpm-libs = 6.1.0-1.fc45"],
            &[
                "/usr/bin/systemd-sysusers",
                "rpmlib(FileDigests) <= 4.6.0-1",
            ],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.contains("systemd-sysusers.x86_64"));
    assert_eq!(orphan_keys(&registros), vec!["zz-controle.x86_64"]);
}

#[test]
fn require_de_arquivo_segura_o_dono_do_script() {
    // `rpm -e --test util-linux-script` falha: linuxtoys exige `/usr/bin/script`,
    // também arquivo do pacote.
    let registros = vec![
        registro(
            "zz-controle.x86_64",
            Reason::Dependency,
            &["zz-controle = 1-1.fc45"],
            &["libc.so.6()(64bit)", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro_com_arquivos(
            "util-linux-script.x86_64",
            Reason::Dependency,
            &[
                "util-linux-script = 2.42.4-1.fc45",
                "util-linux-script(x86-64) = 2.42.4-1.fc45",
            ],
            &["rpmlib(PayloadIsZstd) <= 5.4.18-1"],
            &[
                "/usr/bin/script",
                "/usr/bin/scriptlive",
                "/usr/bin/scriptreplay",
            ],
        ),
        registro(
            "linuxtoys.x86_64",
            Reason::User,
            &["linuxtoys = 7.10.4-1"],
            &["/usr/bin/bash", "/usr/bin/python3", "/usr/bin/script"],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.contains("util-linux-script.x86_64"));
    assert_eq!(orphan_keys(&registros), vec!["zz-controle.x86_64"]);
}

#[test]
fn require_de_arquivo_casa_so_com_o_caminho_exato() {
    // moby-engine também exige `/bin/sh`; se o dono lista só `/usr/bin/sh`,
    // o caminho não casa e o dono continua na lista de órfãos, como no rpm.
    let registros = vec![
        registro_com_arquivos(
            "bash.x86_64",
            Reason::Dependency,
            &["bash = 5.3.3-1.fc45"],
            &["libc.so.6()(64bit)", "rpmlib(FileDigests) <= 4.6.0-1"],
            &["/usr/bin/sh", "/usr/bin/bash"],
        ),
        registro_com_arquivos(
            "tini-static.x86_64",
            Reason::Dependency,
            &[
                "tini-static = 0.19.0-12.fc44",
                "tini-static(x86-64) = 0.19.0-12.fc44",
            ],
            &["rpmlib(FileDigests) <= 4.6.0-1"],
            &["/usr/bin/tini-static"],
        ),
        registro(
            "moby-engine.x86_64",
            Reason::User,
            &["moby-engine = 29.9.3-1.fc45"],
            &[
                "/bin/sh",
                "/usr/bin/tini-static",
                "rpmlib(FileDigests) <= 4.6.0-1",
            ],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.contains("tini-static.x86_64"));
    assert!(
        !requeridos.contains("bash.x86_64"),
        "/bin/sh não deve casar com /usr/bin/sh"
    );
    assert_eq!(orphan_keys(&registros), vec!["bash.x86_64".to_string()]);
}

#[test]
fn provide_de_caminho_segura_quem_nao_e_dono_do_arquivo() {
    // NetworkManager-libreswan exige `/usr/sbin/ipsec`; o pacote `libreswan`
    // o oferece como provide virtual (não é arquivo dele, `rpm -ql` não
    // lista) e `libreswan-minimal` possui o arquivo de verdade. Como no rpm,
    // os dois têm que ficar fora da lista de órfãos.
    let registros = vec![
        registro(
            "zz-controle.x86_64",
            Reason::Dependency,
            &["zz-controle = 1-1.fc45"],
            &["libc.so.6()(64bit)", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro(
            "libreswan.x86_64",
            Reason::Dependency,
            &[
                "/usr/sbin/ipsec",
                "libreswan = 5.4.1-4.fc45",
                "libreswan(x86-64) = 5.4.1-4.fc45",
            ],
            &["rpmlib(PayloadIsZstd) <= 5.4.18-1"],
        ),
        registro_com_arquivos(
            "libreswan-minimal.x86_64",
            Reason::Dependency,
            &["libreswan-minimal = 5.4.1-4.fc45"],
            &["rpmlib(PayloadIsZstd) <= 5.4.18-1"],
            &["/usr/sbin/ipsec"],
        ),
        registro(
            "networkmanager-libreswan.x86_64",
            Reason::User,
            &["networkmanager-libreswan = 1.2.30-4.fc45"],
            &["/usr/sbin/ipsec", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
    ];

    let requeridos = required_keys(&registros);
    assert!(requeridos.contains("libreswan.x86_64"));
    assert!(requeridos.contains("libreswan-minimal.x86_64"));
    assert_eq!(orphan_keys(&registros), vec!["zz-controle.x86_64"]);
}

#[test]
fn else_do_rich_dep_segura_a_alternativa() {
    // `rpm -e --test zed-cli` falha: zed exige
    // `(zed-cli-compat-zfs if zfs else zed-cli)` e `zfs` não está instalado.
    // O braço do `else` era descartado e zed-cli sobrava como órfão.
    let rich = "(zed-cli-compat-zfs if zfs else zed-cli)";
    assert_eq!(
        requirement_names(rich),
        vec![
            "zed-cli-compat-zfs".to_string(),
            "zfs".to_string(),
            "zed-cli".to_string()
        ]
    );

    let registros = vec![
        registro(
            "zz-controle.x86_64",
            Reason::Dependency,
            &["zz-controle = 1-1.fc45"],
            &["libc.so.6()(64bit)", "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
        registro(
            "zed-cli.x86_64",
            Reason::Dependency,
            &["zed-cli = 1.26.2-1.fc45", "zed-cli(x86-64) = 1.26.2-1.fc45"],
            &["rpmlib(PayloadIsZstd) <= 5.4.18-1"],
        ),
        registro(
            "zed.x86_64",
            Reason::User,
            &["zed = 1.26.2-1.fc45"],
            &[rich, "rpmlib(FileDigests) <= 4.6.0-1"],
        ),
    ];

    assert!(required_keys(&registros).contains("zed-cli.x86_64"));
    assert_eq!(orphan_keys(&registros), vec!["zz-controle.x86_64"]);
}

#[test]
fn dono_de_arquivo_ignora_entrada_invalida_e_barra_final() {
    let registros = vec![
        registro(
            "vazio.x86_64",
            Reason::Dependency,
            &["vazio = 1-1.fc45"],
            &[],
        ),
        registro_com_arquivos(
            "a.x86_64",
            Reason::Dependency,
            &["a = 1-1.fc45"],
            &[],
            &["/etc/foo/", "usr/relativo", "   "],
        ),
        registro(
            "usuario.noarch",
            Reason::User,
            &["usuario = 1-1.fc45"],
            &["/etc/foo"],
        ),
    ];

    let donos = file_owners(&registros);
    assert_eq!(donos.get("/etc/foo"), Some(&vec!["a.x86_64".to_string()]));
    assert!(!donos.contains_key("usr/relativo"));

    assert!(required_keys(&registros).contains("a.x86_64"));
    assert_eq!(orphan_keys(&registros), vec!["vazio.x86_64".to_string()]);
}
