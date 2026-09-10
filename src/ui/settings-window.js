document.addEventListener('DOMContentLoaded', () => {
    // Get DOM elements
    const closeButton = document.getElementById('closeButton');
    const quitButton = document.getElementById('quitButton');
    const windowGapInput = document.getElementById('windowGap');
    const codingLanguageSelect = document.getElementById('codingLanguage');
    const activeSkillSelect = document.getElementById('activeSkill');
    const iconGrid = document.getElementById('iconGrid');
    const endpointSilenceInput = document.getElementById('endpointSilenceMs');

    // Check if window.api exists
    if (!window.api) {
        console.error('window.api not available');
        return;
    }

    // Request current settings when window opens
    const requestCurrentSettings = () => {
        if (window.electronAPI && window.electronAPI.getSettings) {
            window.electronAPI.getSettings().then(settings => {
                loadSettingsIntoUI(settings);
            }).catch(error => {
                console.error('Failed to get settings:', error);
            });
        }
    };

    // Close button handler
    if (closeButton) {
        closeButton.addEventListener('click', () => {
            window.api.send('close-settings');
        });
    }

    // Quit button handler with multiple attempts
    if (quitButton) {
        quitButton.addEventListener('click', () => {
            try {
                if (window.api && window.api.send) {
                    window.api.send('quit-app');
                }
                if (window.electronAPI && window.electronAPI.quit) {
                    window.electronAPI.quit();
                }
                setTimeout(() => {
                    window.close();
                }, 500);
            } catch (error) {
                console.error('Error quitting app:', error);
                window.close();
            }
        });
    }

    // Function to load settings into UI
    const loadSettingsIntoUI = (settings) => {
        if (settings.windowGap && windowGapInput) windowGapInput.value = settings.windowGap;
        if (settings.codingLanguage && codingLanguageSelect) codingLanguageSelect.value = settings.codingLanguage;
        if (settings.activeSkill && activeSkillSelect) activeSkillSelect.value = settings.activeSkill;
        if (settings.endpointSilenceMs && endpointSilenceInput) endpointSilenceInput.value = settings.endpointSilenceMs;

        // Handle icon selection
        const selectedIcon = settings.selectedIcon || settings.appIcon;
        if (selectedIcon && iconGrid) {
            const iconOptions = iconGrid.querySelectorAll('.icon-option');
            iconOptions.forEach(option => {
                if (option.dataset.icon === selectedIcon) {
                    option.classList.add('selected');
                } else {
                    option.classList.remove('selected');
                }
            });
        }
    };

    // Load settings when window opens
    window.api.receive('load-settings', (settings) => {
        loadSettingsIntoUI(settings);
    });

    // Listen for settings window shown event
    if (window.electronAPI && window.electronAPI.receive) {
        window.electronAPI.receive('settings-window-shown', () => {
            requestCurrentSettings();
        });
    }

    // Save settings helper function
    const saveSettings = () => {
        const settings = {};
        if (windowGapInput) settings.windowGap = windowGapInput.value;
        if (codingLanguageSelect) settings.codingLanguage = codingLanguageSelect.value;
        if (activeSkillSelect) settings.activeSkill = activeSkillSelect.value;
        if (endpointSilenceInput) settings.endpointSilenceMs = parseInt(endpointSilenceInput.value) || 900;

        window.api.send('save-settings', settings);
    };

    // Add event listeners for all inputs
    const inputs = [
        windowGapInput,
        endpointSilenceInput,
    ];

    inputs.forEach(input => {
        if (input) {
            input.addEventListener('change', saveSettings);
            input.addEventListener('blur', saveSettings);
        }
    });

    // Language selection handler
    if (codingLanguageSelect) {
        codingLanguageSelect.addEventListener('change', (e) => {
            saveSettings();
        });
    }

    // Skill selection handler
    if (activeSkillSelect) {
        activeSkillSelect.addEventListener('change', (e) => {
            saveSettings();
            window.api.send('update-skill', e.target.value);
        });
    }

    // Initialize icon grid with correct paths
    const initializeIconGrid = () => {
        if (!iconGrid) return;

        const icons = [
            { key: 'terminal', name: 'Terminal', src: './assests/icons/terminal.png' },
            { key: 'activity', name: 'Activity', src: './assests/icons/activity.png' },
            { key: 'settings', name: 'Settings', src: './assests/icons/settings.png' }
        ];

        iconGrid.innerHTML = '';

        icons.forEach(icon => {
            const iconElement = document.createElement('div');
            iconElement.className = 'icon-option';
            iconElement.dataset.icon = icon.key;

            const img = document.createElement('img');
            img.src = icon.src;
            img.alt = icon.name;
            img.onerror = () => {
                const altPaths = [
                    `./assests/${icon.key}.png`,
                    `./assets/icons/${icon.key}.png`,
                    `./assets/${icon.key}.png`
                ];

                let pathIndex = 0;
                const tryNextPath = () => {
                    if (pathIndex < altPaths.length) {
                        img.src = altPaths[pathIndex];
                        pathIndex++;
                    } else {
                        img.style.display = 'none';
                    }
                };
                img.onerror = tryNextPath;
                tryNextPath();
            };

            const label = document.createElement('div');
            label.textContent = icon.name;

            iconElement.appendChild(img);
            iconElement.appendChild(label);

            iconElement.addEventListener('click', () => {
                iconGrid.querySelectorAll('.icon-option').forEach(opt => {
                    opt.classList.remove('selected');
                });
                iconElement.classList.add('selected');
                window.api.send('save-settings', { selectedIcon: icon.key });
                iconElement.style.transform = 'scale(0.95)';
                setTimeout(() => {
                    iconElement.style.transform = 'scale(1)';
                }, 100);
            });

            iconGrid.appendChild(iconElement);
        });
    };

    initializeIconGrid();

    setTimeout(() => {
        requestCurrentSettings();
    }, 200);

    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            window.api.send('close-settings');
        }
    });
});
