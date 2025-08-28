/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./templates/new/*.{html,js,ts,tsx}",
    "./static/**/*.{html,js,ts,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'dark-bg': '#0f172a',
        'card-bg': '#1e293b',
        'primary': '#f43f5e',
        'secondary': '#94a3b8'
      }
    }
  },
  plugins: [],
}

