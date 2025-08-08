import { Link } from "react-router-dom";

const Footer = () => {
  return (
    <footer className="border-t border-border/50 bg-background">
      <div className="container mx-auto px-4 py-8">
        <div className="flex flex-col items-center justify-center space-y-4">
          <div className="flex flex-wrap items-center justify-center gap-6 text-sm text-muted-foreground">
            <Link
              to="/terms"
              className="hover:text-foreground transition-colors"
            >
              Terms of Service
            </Link>
            <span className="hidden sm:inline">•</span>
            <span>Play Responsibly</span>
          </div>
          <div className="text-center text-xs text-muted-foreground">
            <p>© 2024 SatsDay. All rights reserved.</p>
            <p className="mt-2">
              Gambling involves risk. Please gamble responsibly and only with
              funds you can afford to lose.
            </p>
          </div>
        </div>
      </div>
    </footer>
  );
};

export default Footer;
