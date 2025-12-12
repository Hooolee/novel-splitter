/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Semantic Backgrounds
        bg: 'var(--bg-main)',
        sidebar: 'var(--bg-sidebar)',
        input: 'var(--bg-input)',
        
        // Semantic Text
        txt: 'var(--text-main)',
        'txt-dim': 'var(--text-dim)',
        'txt-inverse': 'var(--text-inverse)',
        
        // Semantic Borders
        border: 'var(--border-dim)',
        'border-dim': 'var(--border-subtle)', // For very subtle borders
        
        // Interaction / States
        hover: 'var(--bg-hover)',
        active: 'var(--bg-active)',
        subtle: 'var(--bg-subtle)',
        
        // Accents
        accent: 'var(--accent-primary)',
        
        // Keeping card/success if needed, or mapping them
        card: 'var(--bg-sidebar)', // Reuse sidebar color for cards usually in this design
        success: '#22c55e',
      }
    },
  },
  plugins: [],
}
