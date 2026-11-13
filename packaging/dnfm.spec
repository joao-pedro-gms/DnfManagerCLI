Name:           dnfm
Version:        0.3.0
Release:        1%{?dist}
Summary:        Gerenciador de pacotes DNF5 com TUI e CLI

License:        MIT
URL:            https://github.com/joao-pedro-gms/DnfManagerCLI
# RPM de binário: o executável é construído com `cargo build --release` antes
# do rpmbuild e copiado para SOURCES junto com o man page e as completions.
Source0:        dnfm
Source1:        dnfm.1
Source2:        dnfm.bash
Source3:        dnfm.zsh
Source4:        dnfm.fish
Source5:        LICENSE

Requires:       dnf5
Requires:       dnf5daemon-server

%description
dnfm gerencia os pacotes instalados no Fedora através do dnf5daemon.
Permite marcar pacotes para remoção em lote, ver o efeito da transação
(o que sai junto como dependência e quanto espaço é liberado) antes de
aplicá-la, acompanhar o progresso em tempo real e inspecionar
atualizações, histórico, repositórios e órfãos.

As operações de leitura rodam como usuário comum; as de escrita são
autenticadas pelo polkit.

%prep
cp %{SOURCE5} .

%build

%install
install -Dpm0755 %{SOURCE0} %{buildroot}%{_bindir}/dnfm
install -Dpm0644 %{SOURCE1} %{buildroot}%{_mandir}/man1/dnfm.1
install -Dpm0644 %{SOURCE2} %{buildroot}%{_datadir}/bash-completion/completions/dnfm
install -Dpm0644 %{SOURCE3} %{buildroot}%{_datadir}/zsh/site-functions/_dnfm
install -Dpm0644 %{SOURCE4} %{buildroot}%{_datadir}/fish/vendor_completions.d/dnfm.fish

%files
%license LICENSE
%{_bindir}/dnfm
%{_mandir}/man1/dnfm.1*
%{_datadir}/bash-completion/completions/dnfm
%{_datadir}/zsh/site-functions/_dnfm
%{_datadir}/fish/vendor_completions.d/dnfm.fish

%changelog
* Fri Nov 13 2026 Joao Pedro G. M. S. <joaopedrogms@users.noreply.github.com> - 0.2.0-1
- Categorias próprias, agrupamento por categoria/família/repo/motivo, sidebar
  com barras, chips de filtro, animações, temas e painel de filtros ampliado

* Fri Nov 13 2026 Joao Pedro G. M. S. <joaopedrogms@users.noreply.github.com> - 0.1.0-1
- Versão inicial: TUI com marcação em lote, preview de dependências,
  execução via polkit, atualizações com advisories, histórico, repos e
  órfãos; CLI com saída JSON.
