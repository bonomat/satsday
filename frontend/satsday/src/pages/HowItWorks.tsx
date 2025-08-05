import Navbar from "@/components/Navbar";
import HowItWorksContent from "@/components/HowItWorks";

export default function HowItWorksPage() {
  return (
    <div className="min-h-screen bg-background">
      <Navbar />
      <div className="py-16">
        <HowItWorksContent />
      </div>
    </div>
  );
}
