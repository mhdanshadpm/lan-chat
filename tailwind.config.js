/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: ['./index.html', './bubble.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          '-apple-system',
          'BlinkMacSystemFont',
          'Segoe UI',
          'Inter',
          'Roboto',
          'Helvetica Neue',
          'Arial',
          'sans-serif',
        ],
      },
      boxShadow: {
        soft: '0 20px 60px -20px rgba(15, 23, 42, 0.35)',
        bubble: '0 24px 60px -12px rgba(2, 6, 23, 0.55)',
      },
      keyframes: {
        rise: {
          '0%': { opacity: '0', transform: 'translateY(8px) scale(0.985)' },
          '100%': { opacity: '1', transform: 'translateY(0) scale(1)' },
        },
        pop: {
          '0%': { opacity: '0', transform: 'scale(0.92)' },
          '100%': { opacity: '1', transform: 'scale(1)' },
        },
      },
      animation: {
        rise: 'rise 180ms ease-out',
        pop: 'pop 160ms ease-out',
      },
    },
  },
  plugins: [],
};
