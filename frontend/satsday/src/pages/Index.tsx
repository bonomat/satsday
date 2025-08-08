import Navbar from "@/components/Navbar";
import GameHero from "@/components/GameHero";
import GamesOverview from "@/components/GamesOverview";
import Footer from "@/components/Footer";

const Index = () => {
  return (
    <div className="min-h-screen bg-background flex flex-col">
      <Navbar />

      {/* Hero Section */}
      <GameHero />

      {/* Games Overview Section */}
      <section className="border-t border-border/50 flex-grow">
        <GamesOverview />
      </section>

      <Footer />
    </div>
  );
};

export default Index;
