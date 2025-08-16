// Main application JavaScript
document.addEventListener('DOMContentLoaded', function() {
    console.log('maccms-rust frontend loaded');
    
    // Initialize tooltips
    initTooltips();
    
    // Initialize lazy loading
    initLazyLoading();
    
    // Initialize mobile menu
    initMobileMenu();
    
    // Initialize search functionality
    initSearch();
    
    // Initialize video card interactions
    initVideoCards();
});

// Initialize tooltips
function initTooltips() {
    const tooltipElements = document.querySelectorAll('[data-tooltip]');
    tooltipElements.forEach(element => {
        element.addEventListener('mouseenter', function() {
            const tooltip = document.createElement('div');
            tooltip.className = 'absolute bg-gray-800 text-white text-xs rounded py-1 px-2 z-50';
            tooltip.textContent = this.getAttribute('data-tooltip');
            tooltip.style.bottom = '100%';
            tooltip.style.left = '50%';
            tooltip.style.transform = 'translateX(-50%)';
            tooltip.style.marginBottom = '5px';
            
            this.style.position = 'relative';
            this.appendChild(tooltip);
        });
        
        element.addEventListener('mouseleave', function() {
            const tooltip = this.querySelector('.absolute.bg-gray-800');
            if (tooltip) {
                tooltip.remove();
            }
        });
    });
}

// Initialize lazy loading for images
function initLazyLoading() {
    const images = document.querySelectorAll('img[loading="lazy"]');
    
    if ('IntersectionObserver' in window) {
        const imageObserver = new IntersectionObserver((entries, observer) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    const img = entry.target;
                    img.src = img.dataset.src || img.src;
                    img.classList.remove('opacity-0');
                    img.classList.add('opacity-100');
                    observer.unobserve(img);
                }
            });
        });
        
        images.forEach(img => {
            img.classList.add('opacity-0', 'transition-opacity', 'duration-300');
            imageObserver.observe(img);
        });
    } else {
        // Fallback for browsers that don't support IntersectionObserver
        images.forEach(img => {
            img.src = img.dataset.src || img.src;
        });
    }
}

// Initialize mobile menu
function initMobileMenu() {
    const mobileMenuButton = document.getElementById('mobile-menu-button');
    const mobileMenu = document.getElementById('mobile-menu');
    
    if (mobileMenuButton && mobileMenu) {
        mobileMenuButton.addEventListener('click', function() {
            mobileMenu.classList.toggle('hidden');
        });
        
        // Close menu when clicking outside
        document.addEventListener('click', function(event) {
            if (!mobileMenuButton.contains(event.target) && !mobileMenu.contains(event.target)) {
                mobileMenu.classList.add('hidden');
            }
        });
    }
}

// Initialize search functionality
function initSearch() {
    const searchForms = document.querySelectorAll('form[action="/search"]');
    const searchInputs = document.querySelectorAll('input[name="wd"]');
    
    searchInputs.forEach(input => {
        // Add search suggestions
        let searchTimeout;
        
        input.addEventListener('input', function() {
            clearTimeout(searchTimeout);
            const query = this.value.trim();
            
            if (query.length > 1) {
                searchTimeout = setTimeout(() => {
                    showSearchSuggestions(query, this);
                }, 300);
            } else {
                hideSearchSuggestions();
            }
        });
        
        input.addEventListener('focus', function() {
            if (this.value.trim().length > 1) {
                showSearchSuggestions(this.value.trim(), this);
            }
        });
        
        input.addEventListener('blur', function() {
            setTimeout(() => hideSearchSuggestions(), 200);
        });
    });
    
    // Handle form submission
    searchForms.forEach(form => {
        form.addEventListener('submit', function(e) {
            const searchInput = this.querySelector('input[name="wd"]');
            const query = searchInput.value.trim();
            
            if (!query) {
                e.preventDefault();
                searchInput.focus();
                showNotification('请输入搜索关键词', 'warning');
            }
        });
    });
}

// Show search suggestions
function showSearchSuggestions(query, inputElement) {
    // This would typically make an API call to get search suggestions
    // For now, we'll show a loading state
    const suggestionsContainer = document.getElementById('search-suggestions');
    if (!suggestionsContainer) return;
    
    suggestionsContainer.innerHTML = '<div class="p-2 text-gray-500 text-sm">搜索中...</div>';
    suggestionsContainer.classList.remove('hidden');
    
    // Position suggestions below the input
    const rect = inputElement.getBoundingClientRect();
    suggestionsContainer.style.top = (rect.bottom + window.scrollY) + 'px';
    suggestionsContainer.style.left = rect.left + 'px';
    suggestionsContainer.style.width = rect.width + 'px';
    
    // Simulate API call delay
    setTimeout(() => {
        suggestionsContainer.innerHTML = `
            <div class="p-2">
                <div class="text-xs text-gray-500 mb-2">搜索建议</div>
                <a href="/search?wd=${encodeURIComponent(query)}" class="block px-2 py-1 hover:bg-gray-100 rounded text-sm">
                    搜索 "${query}"
                </a>
            </div>
        `;
    }, 500);
}

// Hide search suggestions
function hideSearchSuggestions() {
    const suggestionsContainer = document.getElementById('search-suggestions');
    if (suggestionsContainer) {
        suggestionsContainer.classList.add('hidden');
    }
}

// Initialize video card interactions
function initVideoCards() {
    const videoCards = document.querySelectorAll('.video-card');
    
    videoCards.forEach(card => {
        // Add hover effect
        card.addEventListener('mouseenter', function() {
            this.style.transform = 'translateY(-4px)';
        });
        
        card.addEventListener('mouseleave', function() {
            this.style.transform = 'translateY(0)';
        });
        
        // Add click handler for play button
        const playButton = this.querySelector('.fa-play-circle');
        if (playButton) {
            playButton.addEventListener('click', function(e) {
                e.preventDefault();
                e.stopPropagation();
                
                const link = this.closest('a');
                if (link) {
                    window.location.href = link.href;
                }
            });
        }
    });
}

// Show notification
function showNotification(message, type = 'info') {
    const notification = document.createElement('div');
    notification.className = `fixed top-4 right-4 z-50 px-4 py-2 rounded-lg text-white text-sm shadow-lg transform transition-all duration-300 translate-x-full`;
    
    switch (type) {
        case 'success':
            notification.classList.add('bg-green-500');
            break;
        case 'warning':
            notification.classList.add('bg-yellow-500');
            break;
        case 'error':
            notification.classList.add('bg-red-500');
            break;
        default:
            notification.classList.add('bg-blue-500');
    }
    
    notification.textContent = message;
    document.body.appendChild(notification);
    
    // Animate in
    setTimeout(() => {
        notification.classList.remove('translate-x-full');
    }, 100);
    
    // Remove after 3 seconds
    setTimeout(() => {
        notification.classList.add('translate-x-full');
        setTimeout(() => {
            notification.remove();
        }, 300);
    }, 3000);
}

// Utility functions
function formatNumber(num) {
    if (num >= 10000) {
        return (num / 10000).toFixed(1) + '万';
    }
    return num.toString();
}

function formatDuration(seconds) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hours > 0) {
        return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
}

function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

// Export functions for global use
window.showNotification = showNotification;
window.formatNumber = formatNumber;
window.formatDuration = formatDuration;
window.debounce = debounce;