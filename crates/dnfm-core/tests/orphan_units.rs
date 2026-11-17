//! Testes da heurística de órfãos com capabilities reais do daemon.
//!
//! As strings abaixo foram capturadas de `Rpm.list` no `dnf5daemon-server`
//! desta máquina (8.025 instalados, 312.207 provides, 126.395 requires), não
//! são exemplos inventados: `mesa-vulkan-drivers` e `webkitgtk6.0` apareciam
//! como órfãos justamente por causa do formato.

use dnfm_core::capability::{
    capability_name, is_orphan_reason, orphan_keys, required_keys, requirement_names,
    InstalledRecord,
};
use dnfm_core::model::Reason;

fn registro(key: &str, reason: Reason, provides: &[&str], requires: &[&str]) -> InstalledRecord {
    InstalledRecord {
        key: key.to_string(),
        reason,
        provides: provides.iter().map(|s| s.to_string()).collect(),
        requires: requires.iter().map(|s| s.to_string()).collect(),
    }
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
