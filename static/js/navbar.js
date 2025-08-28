// 通用导航栏JavaScript

// 移动端菜单初始化
function initMobileMenu() {
  const mobileMenuBtn = document.getElementById('mobileMenuBtn');
  const mobileNav = document.getElementById('mobileNav');

  if (!mobileMenuBtn || !mobileNav) return;

  // 移动端菜单按钮点击事件
  mobileMenuBtn.addEventListener('click', function(e) {
    e.preventDefault();
    e.stopPropagation();
    
    mobileNav.classList.toggle('show');
  });

  // 点击页面其他区域关闭移动端菜单
  document.addEventListener('click', function(e) {
    if (!mobileNav.contains(e.target) && !mobileMenuBtn.contains(e.target)) {
      mobileNav.classList.remove('show');
    }
  });

  // 鼠标进入菜单区域时取消关闭
  mobileNav.addEventListener('mouseenter', function() {
    // 保持菜单打开状态
  });

  mobileNav.addEventListener('touchstart', function() {
    // 保持菜单打开状态
  });
}

// 播放记录下拉菜单初始化
function initDropdownMenu() {
  const dropdown = document.querySelector('.dropdown');
  const dropdownToggle = document.querySelector('.dropdown-toggle');
  const dropdownMenu = document.querySelector('.dropdown-menu');
  let isDropdownOpen = false;

  if (!dropdown || !dropdownToggle || !dropdownMenu) return;

  // 点击按钮切换菜单显示
  dropdownToggle.addEventListener('click', function(e) {
    e.preventDefault();
    e.stopPropagation();
    
    isDropdownOpen = !isDropdownOpen;
    
    if (isDropdownOpen) {
      dropdownMenu.classList.add('show');
    } else {
      dropdownMenu.classList.remove('show');
    }
  });

  // 点击页面其他区域关闭下拉菜单
  document.addEventListener('click', function(e) {
    if (!dropdown.contains(e.target)) {
      isDropdownOpen = false;
      dropdownMenu.classList.remove('show');
    }
  });

  // 阻止菜单内部点击事件冒泡
  dropdownMenu.addEventListener('click', function(e) {
    e.stopPropagation();
  });
}

// 搜索功能（可选）
function initSearch() {
  const searchInputs = document.querySelectorAll('.mobile-search input, .search-input');
  
  searchInputs.forEach(input => {
    input.addEventListener('keypress', function(e) {
      if (e.key === 'Enter') {
        const query = this.value.trim();
        if (query) {
          // 这里可以添加搜索逻辑
          console.log('搜索:', query);
          // 例如：window.location.href = `search.html?q=${encodeURIComponent(query)}`;
        }
      }
    });
  });

  const searchButtons = document.querySelectorAll('.mobile-search button, .search-button');
  
  searchButtons.forEach(button => {
    button.addEventListener('click', function() {
      const input = this.parentElement.querySelector('input');
      if (input) {
        const query = input.value.trim();
        if (query) {
          // 这里可以添加搜索逻辑
          console.log('搜索:', query);
          // 例如：window.location.href = `search.html?q=${encodeURIComponent(query)}`;
        }
      }
    });
  });
}

// 播放记录管理（可选）
function addToHistory(title, progress, thumbnail) {
  // 这里可以添加播放记录管理逻辑
  console.log('添加到播放记录:', { title, progress, thumbnail });
}

function getHistory() {
  // 这里可以添加获取播放记录的逻辑
  return [];
}

function clearHistory() {
  // 这里可以添加清除播放记录的逻辑
  console.log('清除播放记录');
}

// 导出函数供其他脚本使用
if (typeof module !== 'undefined' && module.exports) {
  module.exports = {
    initMobileMenu,
    initDropdownMenu,
    initSearch,
    addToHistory,
    getHistory,
    clearHistory
  };
}