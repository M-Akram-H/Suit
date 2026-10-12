# Suit for Fedora. The programs are built before this is run (see build-rpm.sh); this
# file says what goes where, and what Suit needs to find on the system.
#
#   rpmbuild -bb suit.spec --define "srcroot /path/to/Suit" --define "suit_version 0.1.0"

%global debug_package %{nil}

Name:           suit
Version:        %{suit_version}
Release:        1%{?dist}
Summary:        A visual editor for Manim slides
License:        MIT
URL:            https://github.com/M-Akram-H/Suit

# What the window is drawn with. Any Fedora desktop has these already.
Requires:       libxkbcommon
Requires:       libwayland-client
Requires:       vulkan-loader
Recommends:     mesa-vulkan-drivers

# Manim is not a Fedora package. `suit-setup` installs it for one person, with pip, into
# a Python environment of Suit's own, and needs these to do it: the Python the pinned
# versions are known to install on, and what two of Manim's parts are compiled with.
Requires:       python3.12
Requires:       python3.12-devel
Requires:       gcc
Requires:       pkgconf-pkg-config
Requires:       cairo-devel
Requires:       pango-devel

# LaTeX, which Manim typesets formulas with: what its usual formulas use, and no more.
Requires:       texlive-latex
Requires:       texlive-standalone
Requires:       texlive-preview
Requires:       texlive-babel-english
Requires:       texlive-amsmath
Requires:       texlive-amsfonts
Requires:       texlive-dvisvgm
Requires:       texlive-collection-fontsrecommended

%description
Suit is a window in which animated mathematical slides are made by clicking
instead of writing Python: objects are put on a slide, what happens to them is
said step by step, and the result is presented. The drawing is done by Manim
itself, so what is seen is what Manim makes.

After installing, run "suit-setup" once (or choose "Set up Manim for Suit" in
the applications menu) to install Manim for your user account.

%install
install -Dm755 %{srcroot}/target/fedora/release/suit      %{buildroot}%{_bindir}/suit
install -Dm755 %{srcroot}/target/fedora/release/suit-cli  %{buildroot}%{_bindir}/suit-cli
strip --strip-unneeded %{buildroot}%{_bindir}/suit %{buildroot}%{_bindir}/suit-cli
install -Dm755 %{srcroot}/packaging/linux/suit-setup      %{buildroot}%{_bindir}/suit-setup
install -Dm644 %{srcroot}/packaging/linux/suit.desktop    %{buildroot}%{_datadir}/applications/suit.desktop
install -Dm644 %{srcroot}/packaging/linux/suit-setup.desktop %{buildroot}%{_datadir}/applications/suit-setup.desktop
install -Dm644 %{srcroot}/packaging/linux/suit.xml        %{buildroot}%{_datadir}/mime/packages/suit.xml
install -Dm644 %{srcroot}/crates/ui/assets/icon.png       %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/suit.png
install -Dm644 %{srcroot}/requirements.txt                %{buildroot}%{_datadir}/suit/requirements.txt
install -Dm644 %{srcroot}/docker/constraints.txt          %{buildroot}%{_datadir}/suit/constraints.txt
install -Dm644 -t %{buildroot}%{_datadir}/suit/examples   %{srcroot}/examples/*.suit.json
install -Dm644 %{srcroot}/LICENSE                         %{buildroot}%{_licensedir}/suit/LICENSE
install -Dm644 %{srcroot}/README.md                       %{buildroot}%{_docdir}/suit/README.md

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/suit.desktop
desktop-file-validate %{buildroot}%{_datadir}/applications/suit-setup.desktop

%files
%{_bindir}/suit
%{_bindir}/suit-cli
%{_bindir}/suit-setup
%{_datadir}/applications/suit.desktop
%{_datadir}/applications/suit-setup.desktop
%{_datadir}/mime/packages/suit.xml
%{_datadir}/icons/hicolor/256x256/apps/suit.png
%{_datadir}/suit/
%license %{_licensedir}/suit/LICENSE
%doc %{_docdir}/suit/README.md

%changelog
* Mon Oct 12 2026 Mohamed Akram Hlali - 0.1.0-1
- First package.
