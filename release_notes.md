# LaunchVault v0.1.7 — MSI-установщик и пакеты для Linux

## Что нового
- **MSI-установщик для Windows** — теперь Windows можно ставить через стандартный инсталлятор
- **Автоопределение Steam на Windows** — путь к Steam ищется в реестре (`SOFTWARE\WOW6432Node\Valve\Steam`) и в типовых каталогах (`C:\Steam`, `D:\Steam`, `E:\Steam`, `SystemDrive\Steam`)
- **Пакеты .deb и .rpm** для Linux — устанавливаются стандартными менеджерами пакетов
- Windows .exe теперь собирается кросс-компиляцией MinGW

## Установка

### Linux — AppImage
```bash
wget https://github.com/kik4311/launchvault/releases/download/v0.1.7/LaunchVault-x86_64.AppImage
chmod +x LaunchVault-x86_64.AppImage
./LaunchVault-x86_64.AppImage
```

### Linux — .deb (Debian/Ubuntu)
```bash
wget https://github.com/kik4311/launchvault/releases/download/v0.1.7/launchvault_0.1.7-1_amd64.deb
sudo dpkg -i launchvault_0.1.7-1_amd64.deb
```

### Linux — .rpm (Fedora/RHEL)
```bash
wget https://github.com/kik4311/launchvault/releases/download/v0.1.7/launchvault-0.1.7-1.x86_64.rpm
sudo dnf install ./launchvault-0.1.7-1.x86_64.rpm
```

### Windows — MSI
Скачайте `launchvault.msi` и запустите установщик. Поставятся файлы, ярлык в меню «Пуск» и на рабочем столе.

---

**Полный чейнджлог**: https://github.com/kik4311/launchvault/compare/v0.1.6...v0.1.7
